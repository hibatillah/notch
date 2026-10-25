//! The shelf's list of files and the words it shows. The files stay where they are; the shelf only remembers their
//! paths, in a text file with one path per line. Pure apart from reading and writing that file, so it is tested
//! without a window.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// The most characters a placeholder tile's label shows.
const LABEL_LENGTH: usize = 4;

/// The extension in lower case, without the dot.
fn extension(path: &Path) -> String {
    path.extension().map(|extension| extension.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// The text on a tile that has no preview yet: the extension in capitals, cut to four characters. Empty for a folder.
pub fn tile_label(path: &Path, is_folder: bool) -> String {
    if is_folder {
        return String::new();
    }
    extension(path).to_uppercase().chars().take(LABEL_LENGTH).collect()
}

/// The file name shown under a tile.
pub fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

/// "1 file added", "3 files added".
pub fn added_text(count: usize) -> String {
    format!("{count} file{} added", if count == 1 { "" } else { "s" })
}

/// "brief.pdf removed".
pub fn removed_text(name: &str) -> String {
    format!("{name} removed")
}

/// "Shelf · 4": the shelf's size, shown beside the message in the collapsed pill.
pub fn count_text(count: usize) -> String {
    format!("Shelf \u{00b7} {count}")
}

#[derive(Default)]
pub struct Shelf {
    items: Vec<PathBuf>,
}

impl Shelf {
    /// The files saved in `storage`, leaving out those that no longer exist.
    pub fn load(storage: &Path) -> Self {
        let text = fs::read_to_string(storage).unwrap_or_default();
        let items =
            text.lines().map(str::trim).filter(|line| !line.is_empty()).map(PathBuf::from).filter(|path| path.exists());
        Self { items: items.collect() }
    }

    pub fn save(&self, storage: &Path) {
        if let Some(directory) = storage.parent() {
            let _ = fs::create_dir_all(directory);
        }
        let text: String = self.items.iter().map(|path| format!("{}\n", path.display())).collect();
        let _ = fs::write(storage, text);
    }

    pub fn items(&self) -> &[PathBuf] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Adds the files that exist and are not on the shelf yet; returns how many were added.
    pub fn add(&mut self, paths: impl IntoIterator<Item = PathBuf>) -> usize {
        let before = self.items.len();
        for path in paths {
            if path.exists() && !self.items.contains(&path) {
                self.items.push(path);
            }
        }
        self.items.len() - before
    }

    pub fn remove(&mut self, index: usize) -> Option<PathBuf> {
        (index < self.items.len()).then(|| self.items.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("notch-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn file(directory: &Path, name: &str) -> PathBuf {
        let path = directory.join(name);
        fs::write(&path, "x").unwrap();
        path
    }

    #[test]
    fn a_tile_label_is_the_extension_in_capitals_cut_to_four_characters() {
        assert_eq!(tile_label(Path::new("a.pdf"), false), "PDF");
        assert_eq!(tile_label(Path::new("a.markdown"), false), "MARK");
        assert_eq!(tile_label(Path::new("a"), false), "");
        assert_eq!(tile_label(Path::new("a.pdf"), true), "");
    }

    #[test]
    fn file_names_come_from_the_last_part_of_the_path() {
        assert_eq!(file_name(Path::new(r"C:\Users\me\notch-brief.pdf")), "notch-brief.pdf");
    }

    #[test]
    fn the_messages_count_files() {
        assert_eq!(added_text(1), "1 file added");
        assert_eq!(added_text(2), "2 files added");
        assert_eq!(removed_text("brief.pdf"), "brief.pdf removed");
        assert_eq!(count_text(4), "Shelf \u{00b7} 4");
    }

    #[test]
    fn only_existing_files_that_are_not_there_yet_are_added() {
        let directory = temporary_directory("add");
        let (first, second) = (file(&directory, "a.txt"), file(&directory, "b.txt"));
        let mut shelf = Shelf::default();
        assert_eq!(shelf.add([first.clone(), second.clone(), directory.join("missing.txt")]), 2);
        assert_eq!(shelf.add([first, second]), 0);
        assert_eq!(shelf.len(), 2);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn removing_takes_one_file_off() {
        let directory = temporary_directory("remove");
        let mut shelf = Shelf::default();
        shelf.add([file(&directory, "a.txt"), file(&directory, "b.txt")]);
        assert_eq!(shelf.remove(0), Some(directory.join("a.txt")));
        assert_eq!(shelf.remove(5), None);
        assert_eq!(shelf.len(), 1);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn the_shelf_survives_a_restart_without_the_files_that_vanished() {
        let directory = temporary_directory("persist");
        let (kept, gone) = (file(&directory, "kept.txt"), file(&directory, "gone.txt"));
        let storage = directory.join("shelf.txt");
        let mut shelf = Shelf::default();
        shelf.add([kept.clone(), gone.clone()]);
        shelf.save(&storage);
        fs::remove_file(&gone).unwrap();
        assert_eq!(Shelf::load(&storage).items(), &[kept]);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_missing_storage_file_gives_an_empty_shelf() {
        assert_eq!(Shelf::load(Path::new(r"C:\no\such\shelf.txt")).len(), 0);
    }
}
