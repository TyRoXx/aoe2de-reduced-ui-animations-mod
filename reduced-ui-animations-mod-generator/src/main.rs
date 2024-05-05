#[cfg(test)]
use pretty_assertions::assert_eq;
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    str::FromStr,
};
use tracing::{debug, info, info_span};
use tracing_subscriber::fmt::format::FmtSpan;
use xml_dom::level2::{Document, Element, Name, Node, RefNode};

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

fn is_fade_brush_rectangle(node: &RefNode) -> bool {
    let rectangle = Name::from_str("Rectangle").expect("Tried to parse an XML element name");
    if node.node_name() != rectangle {
        return false;
    }
    let maybe_name = node.get_attribute("x:Name");
    match maybe_name {
        Some(name) => name == "Fade",
        None => false,
    }
}

enum PatchStatus {
    Unchanged,
    Changed,
}

fn patch_xaml_recursively(node: &mut RefNode) -> PatchStatus {
    let blur_effect =
        Name::from_str("local:Age2BlurEffect").expect("Tried to parse an XML element name");
    let swipe_effect =
        Name::from_str("local:Age2SwipeEffect").expect("Tried to parse an XML element name");
    let mut result: PatchStatus = PatchStatus::Unchanged;
    for mut child in node.child_nodes() {
        let name = child.node_name();
        if (name == blur_effect) || (name == swipe_effect) {
            info!("Removing child node: {}", child.node_name());
            node.replace_child(
                node.owner_document()
                    .expect("Expected an owner document")
                    .create_comment(&format!(
                        "The mod {} replaced an element here: {}",
                        GENERATED_MOD_NAME, name
                    )),
                child,
            )
            .expect("Tried to replace an element with a comment");
            result = PatchStatus::Changed;
            continue;
        } else if is_fade_brush_rectangle(&child) {
            info!("Rewriting fade brush element");
            // just do it like "0xDB No UI Transitions 1.4"
            child.set_attribute("Canvas.Left", "-1").unwrap();
            child.set_attribute("Canvas.Top", "-1").unwrap();
            child.set_attribute("Fill", "Green").unwrap();
            child.set_attribute("Height", "1").unwrap();
            child.set_attribute("Width", "1").unwrap();
            result = PatchStatus::Changed;
        }
        match patch_xaml_recursively(&mut child) {
            PatchStatus::Unchanged => {}
            PatchStatus::Changed => result = PatchStatus::Changed,
        }
    }
    result
}

fn xml_to_string(root: &RefNode) -> String {
    // TODO: find a deterministic solution. The order of attributes is random because they use HashMap to store them and don't normalize for formatting. Seriously, wtf?
    root.to_string()
}

fn patch_xaml(original_content: &str) -> Option<String> {
    let mut root = xml_dom::parser::read_xml(original_content).expect("Tried to parse XML");
    match patch_xaml_recursively(&mut root) {
        PatchStatus::Unchanged => None,
        PatchStatus::Changed => Some(xml_to_string(&root)),
    }
}

#[test]
fn test_patch_xaml_tiny() {
    assert_eq!(None, patch_xaml(r#"<Test xmlns="test"></Test>"#));
}

#[test]
fn test_patch_xaml_swipe_effect() {
    assert_eq!(
        Some( "<Test xmlns=\"test\" xmlns:local=\"bla\"><!--The mod Reduced UI Animations replaced an element here: local:Age2SwipeEffect--></Test>".to_string()),
        patch_xaml(r#"<Test xmlns="test" xmlns:local="bla"><local:Age2SwipeEffect/></Test>"#)
    );
}

#[test]
fn test_patch_xaml_blur_effect() {
    assert_eq!(
        Some("<Test xmlns=\"test\" xmlns:local=\"bla\"><Canvas.Effect>\\n<!--The mod Reduced UI Animations replaced an element here: local:Age2BlurEffect--></Canvas.Effect></Test>".to_string()),
        patch_xaml(
            r#"<Test xmlns="test" xmlns:local="bla"><Canvas.Effect>\n<local:Age2BlurEffect /></Canvas.Effect></Test>"#
        )
    );
}

#[test]
fn test_patch_xaml_fade_brush() {
    assert_eq!(
        Some(
            "<Test xmlns=\"test\"><!--a fade over the screen, but under the modals--><Rectangle Width=\"1\" Fill=\"Green\" Canvas.Left=\"-1\" Canvas.Top=\"-1\" Height=\"1\" x:Name=\"Fade\" Visibility=\"Hidden\"></Rectangle></Test>"
                .to_string()
        ),
        patch_xaml(
            r#"<Test xmlns="test"><!--a fade over the screen, but under the modals-->
        <Rectangle 
           x:Name="Fade"
           Fill="{Binding ElementName=window, Path=FadeBrush}" 
           Visibility="Hidden"
           Height="{Binding ElementName=window, Path=ActualHeight}"
           Width="{Binding ElementName=window, Path=ActualWidth}" 
           /></Test>"#
        )
    );
}

#[test]
fn test_patch_xaml_two_different_effects() {
    assert_eq!(
       Some(  "<Test xmlns=\"test\" xmlns:local=\"bla\"><Canvas.Effect>\\n<!--The mod Reduced UI Animations replaced an element here: local:Age2BlurEffect--></Canvas.Effect><!--The mod Reduced UI Animations replaced an element here: local:Age2SwipeEffect--></Test>".to_string()),
        patch_xaml(
            r#"<Test xmlns="test" xmlns:local="bla"><Canvas.Effect>\n<local:Age2BlurEffect /></Canvas.Effect><local:Age2SwipeEffect/></Test>"#
        )
    );
}

#[test]
fn test_patch_xaml_same_effect_twice() {
    assert_eq!(
       Some(  "<Test xmlns:local=\"bla\" xmlns=\"test\"><!--The mod Reduced UI Animations replaced an element here: local:Age2SwipeEffect--><!--The mod Reduced UI Animations replaced an element here: local:Age2SwipeEffect--></Test>".to_string()),
        patch_xaml(
            r#"<Test xmlns="test" xmlns:local="bla"><local:Age2SwipeEffect/><local:Age2SwipeEffect/></Test>"#
        )
    );
}

#[test]
fn test_patch_xaml_realistic() {
    assert_eq!(
       Some(  "<local:Age2ScreenSimpleMainMenu xmlns:local=\"clr-namespace:aoe2wpfg\" xmlns:x=\"http://schemas.microsoft.com/winfx/2006/xaml\" xmlns:d=\"http://schemas.microsoft.com/expression/blend/2008\" xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" d:DesignWidth=\"3840\" mc:Ignorable=\"d\" x:Name=\"Page\" d:DesignHeight=\"2160\"><Canvas Height=\"2160\" Background=\"Transparent\" Width=\"3840\"><Canvas.Effect><!--The mod Reduced UI Animations replaced an element here: local:Age2SwipeEffect--></Canvas.Effect></Canvas><Canvas Width=\"1000\" Background=\"Transparent\" Canvas.Left=\"235\" Height=\"2160\"><Canvas.Effect><!--The mod Reduced UI Animations replaced an element here: local:Age2BlurEffect--></Canvas.Effect></Canvas></local:Age2ScreenSimpleMainMenu>".to_string()),
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

fn modify_xaml_file(original_content: &[u8]) -> Option<Vec<u8>> {
    let original_content_string = encoding_rs::UTF_8
        .decode_with_bom_removal(original_content)
        .0;
    let modified_content = patch_xaml(original_content_string.as_ref());
    modified_content.map(|value| value.into())
}

fn modify_xaml_files<'t>(directory: &'t dyn ReadDirectory) -> BTreeMap<String, DirectoryEntry> {
    let mut entries = BTreeMap::new();
    for file_entry in directory.enumerate_files() {
        let maybe_modified = modify_xaml_file(&file_entry.content);
        match maybe_modified {
            Some(modified) => {
                info!("XAML file will be replaced: {}", &file_entry.name);
                entries.insert(file_entry.name, DirectoryEntry::File(modified));
            }
            None => info!("XAML file needs no changes: {}", &file_entry.name),
        }
    }
    entries
}

fn modify_wpfg<'t>(wpfg_installation: &'t (dyn ReadDirectory + 't)) -> Directory {
    let mut entries = modify_xaml_files(wpfg_installation);
    for subdirectory in ["dialog", "panel", "screen", "tab"] {
        let _span = info_span!("Modding", subdirectory);
        let subdirectory_reader = wpfg_installation.subdirectory(subdirectory);
        let modified_files = modify_xaml_files(subdirectory_reader.as_ref());
        entries.insert(
            subdirectory.to_string(),
            DirectoryEntry::Subdirectory(Box::new(Directory {
                entries: modified_files,
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
