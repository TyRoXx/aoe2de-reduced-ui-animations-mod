#[cfg(test)]
use pretty_assertions::assert_eq;
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tracing::{debug, info, info_span};
use tracing_subscriber::fmt::format::FmtSpan;

struct FileEntry {
    name: String,
    content: Vec<u8>,
}

trait ReadDirectory {
    fn subdirectory(&self, name: &str) -> Box<dyn ReadDirectory>;
    fn enumerate_files(&self) -> Box<dyn Iterator<Item = FileEntry>>;
}

struct FileSystem {
    root: PathBuf,
}

impl ReadDirectory for FileSystem {
    fn subdirectory(&self, name: &str) -> Box<dyn ReadDirectory> {
        Box::new(FileSystem {
            root: self.root.join(name),
        })
    }

    fn enumerate_files(&self) -> Box<dyn Iterator<Item = FileEntry>> {
        debug!("Enumerating directory entries of {}", self.root.display());
        let enumerating = std::fs::read_dir(&self.root).expect("Tried to enumerate directory");
        let iterator = enumerating.filter_map(|entry_result| {
            let entry = entry_result.expect("Tried to look at enumerated directory entry");
            let entry_type = entry
                .file_type()
                .expect("Tried to determine type of directory entry");
            let path = entry.path();
            if !entry_type.is_file() {
                debug!("Ignoring non-file directory entry: {}", path.display());
                return None;
            }
            let name = entry
                .file_name()
                .into_string()
                .expect("Tried to convert file name to string");
            let content = std::fs::read(&path).expect("Tried to read a file");
            debug!("Read file {} with size {}", &name, content.len());
            Some(FileEntry {
                name: name,
                content: content,
            })
        });
        Box::new(iterator)
    }
}

trait WriteDirectory {
    fn subdirectory(&self, name: &str) -> Box<dyn WriteDirectory>;
    fn create_file(&self, name: &str, content: &[u8]);
}

impl WriteDirectory for FileSystem {
    fn subdirectory(&self, name: &str) -> Box<dyn WriteDirectory> {
        Box::new(FileSystem {
            root: self.root.join(name),
        })
    }

    fn create_file(&self, name: &str, content: &[u8]) {
        std::fs::create_dir_all(&self.root).expect("Tried to create a directory");
        let file_path = self.root.join(name);
        debug!(
            "Creating file {} with {} bytes of content",
            file_path.display(),
            content.len()
        );
        std::fs::write(&file_path, &content).expect("Tried to create a file");
    }
}

enum DirectoryEntry {
    File(Vec<u8>),
    Subdirectory(Box<Directory>),
}

struct Directory {
    entries: BTreeMap<String, DirectoryEntry>,
}

fn write_directory(data: &Directory, into: &dyn WriteDirectory) {
    for entry in &data.entries {
        let entry_name = entry.0;
        match entry.1 {
            DirectoryEntry::File(content) => into.create_file(&entry_name, &content[..]),
            DirectoryEntry::Subdirectory(subdirectory) => write_directory(
                &subdirectory.as_ref(),
                into.subdirectory(&entry_name).as_ref(),
            ),
        }
    }
}

const GENERATED_MOD_NAME: &str = "Reduced UI Animations";

fn replace_all_matching_elements(
    original_content: &str,
    start_pattern: &str,
    end_pattern: &str,
) -> String {
    // We use string replacement instead of an XML parser to preserve all of the whitespace in order to make diffing easier.
    let mut modified_content = String::new();
    let mut remaining_content: &str = original_content;
    while !remaining_content.is_empty() {
        match remaining_content.find(start_pattern) {
            Some(start_found_at) => {
                debug!("Found {}", start_pattern);
                let (before_start_pattern, at_start_pattern) =
                    remaining_content.split_at(start_found_at);
                modified_content += before_start_pattern;
                modified_content += "<!--Commented out by the mod ";
                modified_content += GENERATED_MOD_NAME;
                modified_content += ": ";
                modified_content += start_pattern;
                let (_, after_element_start_pattern) =
                    at_start_pattern.split_at(start_pattern.len());
                let end_found_at = after_element_start_pattern
                    .find(end_pattern)
                    .expect("Expected to find the end of the XML element");
                let (element_content, after_element_content) =
                    after_element_start_pattern.split_at(end_found_at);
                modified_content += element_content;
                modified_content += end_pattern;
                modified_content += "-->";
                (_, remaining_content) = after_element_content.split_at(end_pattern.len());
            }
            None => {
                modified_content += remaining_content;
                remaining_content = "";
            }
        }
    }
    // we don't remove anything, we just comment out, so that you can still see what had been there
    assert!(modified_content.len() >= original_content.len());
    modified_content
}

fn patch_xaml(original_content: &str) -> String {
    let no_swipe_effects =
        replace_all_matching_elements(original_content, "<local:Age2SwipeEffect", "/>");
    // not sure what Age2BlurEffect is, but "0xDB No UI Transitions 1.4" comments it out
    let no_blur_effects =
        replace_all_matching_elements(&no_swipe_effects, "<local:Age2BlurEffect", "/>");
    no_blur_effects
}

#[test]
fn test_patch_xaml_empty() {
    assert_eq!("", patch_xaml(""));
}

#[test]
fn test_patch_xaml_swipe_effect() {
    assert_eq!(
        "<!--Commented out by the mod Reduced UI Animations: <local:Age2SwipeEffect/>-->",
        patch_xaml("<local:Age2SwipeEffect/>")
    );
}

#[test]
fn test_patch_xaml_blur_effect() {
    assert_eq!(
        "<Canvas.Effect>\n<!--Commented out by the mod Reduced UI Animations: <local:Age2BlurEffect />--></Canvas.Effect>",
        patch_xaml("<Canvas.Effect>\n<local:Age2BlurEffect /></Canvas.Effect>")
    );
}

#[test]
fn test_patch_xaml_two_different_effects() {
    assert_eq!(
        "<Canvas.Effect>\n<!--Commented out by the mod Reduced UI Animations: <local:Age2BlurEffect />--></Canvas.Effect><!--Commented out by the mod Reduced UI Animations: <local:Age2SwipeEffect/>-->",
        patch_xaml("<Canvas.Effect>\n<local:Age2BlurEffect /></Canvas.Effect><local:Age2SwipeEffect/>")
    );
}

#[test]
fn test_patch_xaml_same_effect_twice() {
    assert_eq!(
        "<!--Commented out by the mod Reduced UI Animations: <local:Age2SwipeEffect/>--><!--Commented out by the mod Reduced UI Animations: <local:Age2SwipeEffect/>-->",
        patch_xaml("<local:Age2SwipeEffect/><local:Age2SwipeEffect/>")
    );
}

#[test]
fn test_patch_xaml_realistic() {
    assert_eq!(
        r#"<local:Age2ScreenSimpleMainMenu x:Name="Page" d:DesignHeight="2160" d:DesignWidth="3840" mc:Ignorable="d" xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" xmlns:d="http://schemas.microsoft.com/expression/blend/2008" xmlns:local="clr-namespace:aoe2wpfg" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
    <Canvas Width="3840" Height="2160" Background="Transparent">
        <Canvas.Effect>
            <!--Commented out by the mod Reduced UI Animations: <local:Age2SwipeEffect
                SwipeLow="{Binding ElementName=Page,Path=SwipeLow}"
                SwipeHigh="{Binding ElementName=Page,Path=SwipeHigh}"
                PixelWidth="3840"
                PixelHeight="2160"
                ScreenWidth="{Binding ElementName=Page, Path=ActualWidth}"
                ScreenHeight="{Binding ElementName=Page, Path=ActualHeight}"
                />-->
        </Canvas.Effect>
    </Canvas>
    
    <Canvas Width="1000" Height="2160" Canvas.Left="235" Background="Transparent">
        <Canvas.Effect>
            <!--Commented out by the mod Reduced UI Animations: <local:Age2BlurEffect
                BlurMask ="{StaticResource ribbon00_BBAA_blurmask}"
                SwipeLow="{Binding ElementName=Page,Path=SwipeLow}"
                SwipeHigh="{Binding ElementName=Page,Path=SwipeHigh}"
                PixelTop="0"
                PixelLeft="235"
                PixelWidth="1000"
                PixelHeight="2160"
                P1="40,0"
                P2="40,0"
                TextureSize="128,128"
                ScreenWidth="{Binding ElementName=Page, Path=ActualWidth}"
                ScreenHeight="{Binding ElementName=Page, Path=ActualHeight}"
                    />-->
        </Canvas.Effect>
    </Canvas>
</local:Age2ScreenSimpleMainMenu>
"#,
        patch_xaml(
            r#"<local:Age2ScreenSimpleMainMenu x:Name="Page" d:DesignHeight="2160" d:DesignWidth="3840" mc:Ignorable="d" xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" xmlns:d="http://schemas.microsoft.com/expression/blend/2008" xmlns:local="clr-namespace:aoe2wpfg" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
    <Canvas Width="3840" Height="2160" Background="Transparent">
        <Canvas.Effect>
            <local:Age2SwipeEffect
                SwipeLow="{Binding ElementName=Page,Path=SwipeLow}"
                SwipeHigh="{Binding ElementName=Page,Path=SwipeHigh}"
                PixelWidth="3840"
                PixelHeight="2160"
                ScreenWidth="{Binding ElementName=Page, Path=ActualWidth}"
                ScreenHeight="{Binding ElementName=Page, Path=ActualHeight}"
                />
        </Canvas.Effect>
    </Canvas>
    
    <Canvas Width="1000" Height="2160" Canvas.Left="235" Background="Transparent">
        <Canvas.Effect>
            <local:Age2BlurEffect
                BlurMask ="{StaticResource ribbon00_BBAA_blurmask}"
                SwipeLow="{Binding ElementName=Page,Path=SwipeLow}"
                SwipeHigh="{Binding ElementName=Page,Path=SwipeHigh}"
                PixelTop="0"
                PixelLeft="235"
                PixelWidth="1000"
                PixelHeight="2160"
                P1="40,0"
                P2="40,0"
                TextureSize="128,128"
                ScreenWidth="{Binding ElementName=Page, Path=ActualWidth}"
                ScreenHeight="{Binding ElementName=Page, Path=ActualHeight}"
                    />
        </Canvas.Effect>
    </Canvas>
</local:Age2ScreenSimpleMainMenu>
"#
        )
    );
}

fn modify_xaml_file(original_content: &[u8]) -> Vec<u8> {
    let original_content_string =
        std::str::from_utf8(original_content).expect("Tried to decode file as UTF-8");
    let modified_content = patch_xaml(original_content_string);
    modified_content.into()
}

fn modify_xaml_files<'t>(directory: &'t dyn ReadDirectory) -> BTreeMap<String, Vec<u8>> {
    let mut entries = BTreeMap::new();
    for file_entry in directory.enumerate_files() {
        let modified_file = modify_xaml_file(&file_entry.content);
        if &file_entry.content[..] == &modified_file[..] {
            info!("XAML file needs no changes: {}", &file_entry.name);
            continue;
        }
        info!("XAML file will be replaced: {}", &file_entry.name);
        entries.insert(file_entry.name, modified_file);
    }
    entries
}

fn modify_wpfg<'t>(wpfg_installation: &'t (dyn ReadDirectory + 't)) -> Directory {
    let mut entries = BTreeMap::new();
    for subdirectory in ["dialog", "panel", "screen", "tab"] {
        let _span = info_span!("Modding", subdirectory);
        let subdirectory_reader = wpfg_installation.subdirectory(subdirectory);
        let mut modified_files = modify_xaml_files(subdirectory_reader.as_ref());
        let subdirectory_entries = modified_files
            .iter_mut()
            .map(|file_entry| {
                (
                    file_entry.0.clone(),
                    DirectoryEntry::File(file_entry.1.clone()),
                )
            })
            .collect();
        entries.insert(
            subdirectory.to_string(),
            DirectoryEntry::Subdirectory(Box::new(Directory {
                entries: subdirectory_entries,
            })),
        );
    }
    Directory { entries: entries }
}

fn create_info_json() -> serde_json::Value {
    let info = json!({
        "Author": "Flauschfuchs",
        "CacheStatus": 0,
        "Description": "Recreation of <b>0xDB No UI Transitions 1.4</b> by Flauschfuchs so that it works in May 2024.",
        "Title": "Reduced UI Animations"
    });
    return info;
}

#[test]
fn test_create_info_json() {
    assert_eq!(
        r#"{"Author":"Flauschfuchs","CacheStatus":0,"Description":"Recreation of <b>0xDB No UI Transitions 1.4</b> by Flauschfuchs so that it works in May 2024.","Title":"Reduced UI Animations"}"#,
        create_info_json().to_string()
    );
}

fn generate_mod(game_installation: &dyn ReadDirectory) -> Directory {
    let mut entries = BTreeMap::new();

    {
        let info_json_name = "info.json";
        info!("Creating {}", info_json_name);
        entries.insert(
            info_json_name.to_string(),
            DirectoryEntry::File(Vec::from(create_info_json().to_string().as_bytes())),
        );
    }

    // TODO: thumbnail.png

    let resources_directory_name = "resources";
    let resources = game_installation.subdirectory(resources_directory_name);

    let common_directory_name = "_common";
    let common = resources.subdirectory(common_directory_name);

    let wpfg_directory_name = "wpfg";
    let wpfg = common.subdirectory(wpfg_directory_name);

    let _span = info_span!("Modding wpfg");
    let modified = modify_wpfg(wpfg.as_ref());
    entries.insert(
        resources_directory_name.to_string(),
        DirectoryEntry::Subdirectory(Box::new(Directory {
            entries: BTreeMap::from([(
                common_directory_name.to_string(),
                DirectoryEntry::Subdirectory(Box::new(Directory {
                    entries: BTreeMap::from([(
                        wpfg_directory_name.to_string(),
                        DirectoryEntry::Subdirectory(Box::new(modified)),
                    )]),
                })),
            )]),
        })),
    );

    Directory { entries: entries }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_span_events(FmtSpan::FULL)
        .with_target(false)
        .init();
    let _span = info_span!("Mod generator");
    let aoe2de_installation = Path::new("C:/Program Files (x86)/Steam/steamapps/common/AoE2DE");
    let user_name = whoami::username();
    let home = Path::new("C:/Users").join(user_name);
    let aoe2_profile_id = "76561197988848434";
    let local_mods = home
        .join("Games/Age of Empires 2 DE")
        .join(aoe2_profile_id)
        .join("mods/local");
    let destination_directory = local_mods.join(GENERATED_MOD_NAME);
    info!("Aoe2 DE installation: {}", aoe2de_installation.display());
    info!("Generating mod into {}", destination_directory.display());
    let generated_mod = generate_mod(&FileSystem {
        root: aoe2de_installation.into(),
    });
    match std::fs::metadata(&destination_directory) {
        Ok(exists) => {
            assert!(exists.is_dir());
            info!(
                "Clearing destination directory {}",
                destination_directory.display()
            );
            std::fs::remove_dir_all(&destination_directory)
                .expect("Tried to delete destination directory");
        }
        Err(error) => info!(
            "Destination directory does not exist yet at {} ({}).",
            destination_directory.display(),
            error
        ),
    }
    info!(
        "Writing the mod to the destination directory: {}",
        destination_directory.display()
    );
    write_directory(
        &generated_mod,
        &FileSystem {
            root: destination_directory,
        },
    )
}
