use super::NordVPN;
use super::{ConfigurationChoice, OpenVpnProvider};
use crate::config::providers::{Input, Password, UiClient};
use crate::config::vpn::OpenVpnProtocol;
use crate::util::delete_all_files_in_dir;
use anyhow::anyhow;
use log::{debug, info, warn};
use regex::Regex;
use std::env;
use std::fmt::Display;
use std::fs::File;
use std::fs::create_dir_all;
use std::io::{Cursor, Read, Write};
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use zip::ZipArchive;

impl OpenVpnProvider for NordVPN {
    fn provider_dns(&self) -> Option<Vec<IpAddr>> {
        Some(vec![
            IpAddr::V4(Ipv4Addr::new(103, 86, 96, 100)),
            IpAddr::V4(Ipv4Addr::new(103, 86, 99, 100)),
        ])
    }

    fn prompt_for_auth(&self, uiclient: &dyn UiClient) -> anyhow::Result<(String, String)> {
        match (env::var("NORDVPN_USERNAME"), env::var("NORDVPN_PASSWORD")) {
            (Ok(username), Ok(password)) if !username.is_empty() && !password.is_empty() => {
                info!("Using NordVPN credentials from environment variables.");
                Ok((username, password))
            }
            _ => {
                if uiclient.is_interactive() {
                    debug!("NordVPN credentials not found in environment variables or are incomplete. Prompting user.");
                    let username = uiclient.get_input(Input {
                        prompt: "NordVPN username".to_string(),
                        validator: None,
                    })?;

                    let password = uiclient.get_password(Password {
                        prompt: "Password".to_string(),
                        confirm: true,
                    })?;
                    Ok((username, password))
                } else {
                    Err(anyhow!("NordVPN credentials (NORDVPN_USERNAME, NORDVPN_PASSWORD) not found in environment variables for non-interactive mode."))
                }
            }
        }
    }

    fn auth_file_path(&self) -> anyhow::Result<Option<PathBuf>> {
        Ok(Some(self.openvpn_dir()?.join("auth.txt")))
    }

    fn create_openvpn_config(&self, uiclient: &dyn UiClient) -> anyhow::Result<()> {
        let openvpn_dir = self.openvpn_dir()?;
        let country_map = crate::util::country_map::code_to_country_map();
        create_dir_all(&openvpn_dir)?;
        delete_all_files_in_dir(&openvpn_dir)?;

        let mut source_url = "https://downloads.nordcdn.com/configs/archives/servers/ovpn.zip".to_string();
        let mut is_github_repo = false;

        if let Ok(alt_url_env) = env::var("NORDVPN_ALT_CONFIG_URL") {
            if !alt_url_env.is_empty() {
                info!("Using alternative NordVPN config URL from environment: {}", alt_url_env);
                if alt_url_env.ends_with(".zip") {
                    source_url = alt_url_env;
                    // We can infer it's a GitHub repo if the URL contains github.com,
                    // useful for path stripping later, even if it's a direct zip link.
                    if alt_url_env.contains("github.com") {
                        is_github_repo = true; // It's a zip, possibly from GitHub
                    }
                } else if alt_url_env.starts_with("https://github.com/") {
                    // It's a GitHub repo URL, not a direct zip link. Transform it.
                    let parts: Vec<&str> = alt_url_env.trim_end_matches('/').split('/').collect();
                    if parts.len() >= 5 { // https: / / github.com / user / repo
                        // Default to 'main' branch. User must provide full .zip URL for other branches.
                        source_url = format!("https://github.com/{}/{}/archive/refs/heads/main.zip", parts[3], parts[4]);
                        info!("Transformed GitHub repository URL to main branch ZIP: {}", source_url);
                        is_github_repo = true;
                    } else {
                        warn!("NORDVPN_ALT_CONFIG_URL looks like a GitHub repository URL but is malformed. Using it directly: {}", alt_url_env);
                        source_url = alt_url_env; // May or may not be a valid ZIP URL
                    }
                } else {
                    // Not ending in .zip and not a github.com repo URL, assume it's a direct ZIP URL.
                    source_url = alt_url_env;
                }
            }
        }

        let config_choice = ConfigType::index_to_variant(
            uiclient.get_configuration_choice(&ConfigType::default())?,
        );
        debug!("Requesting NordVPN configs from URL: {}", source_url);
        let zipfile_response = reqwest::blocking::get(&source_url)?;
        if !zipfile_response.status().is_success() {
            return Err(anyhow!("Failed to download NordVPN configs from {}: {}", source_url, zipfile_response.status()));
        }
        let zip_bytes = zipfile_response.bytes()?;
        let mut zip = ZipArchive::new(Cursor::new(zip_bytes))?;

        let protocol_dir_name = match config_choice.get_protocol() {
            OpenVpnProtocol::TCP => "ovpn_tcp",
            OpenVpnProtocol::UDP => "ovpn_udp",
        };

        let server_regex = Regex::new(r"([a-z]+)(?:-(onion|[a-z]+))?([0-9]+)").unwrap();
        let mut found_files = false;

        for i in 0..zip.len() {
            let mut file_contents: Vec<u8> = Vec::with_capacity(2048);

            let mut file = zip.by_index(i)?;
            if file.is_dir() {
                continue;
            }

            let full_path_str = file.name().to_string();
            let path_parts: Vec<&str> = full_path_str.split('/').collect();

            // Determine effective path parts for matching, skipping potential repo root dir
            let relevant_path_parts = if is_github_repo && path_parts.len() > 1 {
                // If it's a github repo and path has more than one part,
                // assume the first part is the repo-branch name and skip it.
                &path_parts[1..]
            } else {
                &path_parts[..]
            };

            if relevant_path_parts.is_empty() || relevant_path_parts[0] != protocol_dir_name {
                continue;
            }

            // Check if the file extension is .ovpn
            if !full_path_str.ends_with(".ovpn") {
                continue;
            }

            found_files = true; // Mark that we've found relevant files to process
            file.read_to_end(&mut file_contents)?;

            // Extract the simple filename (e.g., "us1234.nordvpn.com.udp.ovpn")
            let simple_filename = relevant_path_parts.last().unwrap_or(&"").to_string();
            if simple_filename.is_empty() {
                debug!("Skipping empty filename derived from path: {}", full_path_str);
                continue;
            }

            let server_name_for_regex = simple_filename.split('.').next().unwrap_or("").to_lowercase();

            let output_filename = if let Some(cap) = server_regex.captures(&server_name_for_regex) {
                // Check special config type (Onion, DoubleVPN)
                // The regex captures group 2 for this. e.g. us-onion123 -> group 2 is "onion"
                // For the alternative repo, filenames are like "ad1.nordvpn.com.udp.ovpn",
                // so group 2 will likely be None unless the server name itself contains "onion" or similar.
                let special_type_in_name = cap.get(2).map(|m| m.as_str());

                if config_choice.is_onion() {
                    if special_type_in_name != Some("onion") { // Strict check for "onion"
                        debug!("Skipping file {} (server name {}) as it's not an Onion server but Onion config type was chosen.", full_path_str, server_name_for_regex);
                        continue;
                    }
                } else if config_choice.is_double() {
                     // For DoubleVPN, the official zip might have names like country1-country2.nordvpn.com
                     // The regex might capture the first country. If `special_type_in_name` is Some and not "onion",
                     // it might indicate a double VPN like "us-ca".
                     // If the alternative repo doesn't follow this, this check might not be effective.
                    if special_type_in_name.is_none() || special_type_in_name == Some("onion") {
                        debug!("Skipping file {} (server name {}) as it's not a DoubleVPN server but DoubleVPN config type was chosen.", full_path_str, server_name_for_regex);
                        continue;
                    }
                } else { // Standard config type chosen
                    if special_type_in_name.is_some() {
                        debug!("Skipping file {} (server name {}) as it seems to be a special type server (e.g., Onion/Double) but standard config type was chosen.", full_path_str, server_name_for_regex);
                        continue;
                    }
                }

                // Group 1 is the country code or first part of server name.
                if let Some(code_match) = cap.get(1) {
                    let code = code_match.as_str();
                    let country_display_name = country_map.get(code).cloned().unwrap_or_else(|| {
                        debug!("Could not map country code '{}' from server name '{}' to a full name. Using code.", code, server_name_for_regex);
                        code
                    });
                    // Use the full server_name_for_regex for uniqueness in filename, replace '-' with '_'
                    let server_identifier = server_name_for_regex.replace('-', "_");
                    format!("{}-{}.ovpn", country_display_name, server_identifier)
                } else {
                    debug!("Filename {} did not match expected pattern (no country code part). Using original simple filename.", full_path_str);
                    simple_filename
                }
            } else {
                debug!("Filename {} (server name {}) did not match established server regex. Using original simple filename.", full_path_str, server_name_for_regex);
                simple_filename
            };


            debug!("Processing file: {} as {}", full_path_str, output_filename);
            let mut outfile = File::create(openvpn_dir.join(output_filename.to_lowercase().replace(' ', "_")))?;
            outfile.write_all(&file_contents)?;
        }

        if !found_files {
            warn!("No OpenVPN configuration files found for the selected protocol ({}) and source ({}). This might be due to an incorrect NORDVPN_ALT_CONFIG_URL, an empty/wrongly structured ZIP, or choosing a special server type not present in the source.", protocol_dir_name, source_url);
        }

        // Write OpenVPN credentials file
        let (user, pass) = self.prompt_for_auth(uiclient)?;
        let auth_file = self.auth_file_path()?;
        if auth_file.is_some() {
            let mut outfile = File::create(auth_file.unwrap())?;
            write!(outfile, "{user}\n{pass}")?;
        }
        Ok(())
    }
}

#[derive(EnumIter, PartialEq)]
enum ConfigType {
    DefaultTcp,
    Udp,
    OnionTcp,
    OnionUdp,
    DoubleTcp,
    DoubleUdp,
}

impl ConfigType {
    fn get_protocol(&self) -> OpenVpnProtocol {
        match self {
            Self::DefaultTcp => OpenVpnProtocol::TCP,
            Self::Udp => OpenVpnProtocol::UDP,
            Self::OnionTcp => OpenVpnProtocol::TCP,
            Self::OnionUdp => OpenVpnProtocol::UDP,
            Self::DoubleTcp => OpenVpnProtocol::TCP,
            Self::DoubleUdp => OpenVpnProtocol::UDP,
        }
    }

    fn is_onion(&self) -> bool {
        matches!(self, Self::OnionTcp | Self::OnionUdp)
    }

    fn is_double(&self) -> bool {
        matches!(self, Self::DoubleTcp | Self::DoubleUdp)
    }
    fn index_to_variant(index: usize) -> Self {
        Self::iter().nth(index).expect("Invalid index")
    }
}

impl Display for ConfigType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::DefaultTcp => "Default (TCP)",
            Self::Udp => "UDP",
            Self::OnionTcp => "Onion (TCP)",
            Self::OnionUdp => "Onion (UDP)",
            Self::DoubleTcp => "Double TCP",
            Self::DoubleUdp => "Double UDP",
        };
        write!(f, "{s}")
    }
}

impl Default for ConfigType {
    fn default() -> Self {
        Self::DefaultTcp
    }
}

impl ConfigurationChoice for ConfigType {
    fn prompt(&self) -> String {
        "Please choose the set of OpenVPN configuration files you wish to install".to_string()
    }

    fn all_names(&self) -> Vec<String> {
        Self::iter().map(|x| format!("{x}")).collect()
    }
    fn all_descriptions(&self) -> Option<Vec<String>> {
        Some(Self::iter().map(|x| x.description().unwrap()).collect())
    }
    fn description(&self) -> Option<String> {
        Some( match self {
            Self::DefaultTcp => "These files connect over TCP.",
            Self::Udp => "These files connect over UDP.",
            Self::OnionTcp => "These files connect via Onion over VPN, using TCP.",
            Self::OnionUdp => "These files connect via Onion over VPN, using UDP.",
            Self::DoubleTcp => "These files connect over TCP across two separate VPN servers. The country listed is the initial outgoing VPN server.",
            Self::DoubleUdp => "These files connect over UDP across two separate VPN servers. The country listed is the initial outgoing VPN server.",
        }.to_string())
    }
}
