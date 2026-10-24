//! The shelf's list of files and how each is shown. The files stay where they are; the shelf only remembers their
//! paths, in a text file with one path per line. Pure apart from reading and writing that file, so it is tested
//! without a window.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::gfx::Color;

/// What kind of thing a file is, which decides the color of its tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Pdf,
    Spreadsheet,
    Document,
    Presentation,
    Archive,
    Image,
    Folder,
    Other,
}

const EXTENSIONS: [(&str, Category); 24] = [
    ("pdf", Category::Pdf),
    ("xlsx", Category::Spreadsheet),
    ("xls", Category::Spreadsheet),
    ("csv", Category::Spreadsheet),
    ("docx", Category::Document),
    ("doc", Category::Document),
    ("txt", Category::Document),
    ("md", Category::Document),
    ("pptx", Category::Presentation),
    ("ppt", Category::Presentation),
    ("zip", Category::Archive),
    ("rar", Category::Archive),
    ("7z", Category::Archive),
    ("png", Category::Image),
    ("jpg", Category::Image),
    ("jpeg", Category::Image),
    ("gif", Category::Image),
    ("webp", Category::Image),
    ("bmp", Category::Image),
    ("svg", Category::Image),
    ("heic", Category::Image),
    ("tif", Category::Image),
    ("tiff", Category::Image),
    ("avif", Category::Image),
];

/// The most characters a tile's label shows.
const LABEL_LENGTH: usize = 4;

pub fn category(path: &Path, is_folder: bool) -> Category {
    if is_folder {
        return Category::Folder;
    }
    let extension = extension(path);
    EXTENSIONS.iter().find(|(known, _)| *known == extension).map_or(Category::Other, |(_, category)| *category)
}

/// The extension in lower case, without the dot.
fn extension(path: &Path) -> String {
    path.extension().map(|extension| extension.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// The text on a tile: the extension in capitals, cut to four characters. Empty for a folder.
pub fn tile_label(path: &Path, is_folder: bool) -> String {
    if is_folder {
        return String::new();
    }
    extension(path).to_uppercase().chars().take(LABEL_LENGTH).collect()
}

pub fn tile_color(category: Category) -> Color {
    match category {
        Category::Pdf => Color::rgb(0.788, 0.165, 0.165),
        Category::Spreadsheet => Color::rgb(0.169, 0.541, 0.243),
        Category::Document => Color::rgb(0.110, 0.494, 0.839),
        Category::Presentation => Color::rgb(0.910, 0.349, 0.047),
        Category::Archive => Color::rgb(0.404, 0.267, 0.851),
        Category::Image => Color::rgb(0.212, 0.310, 0.780),
        Category::Folder => Color::rgb(0.38, 0.38, 0.43),
        Category::Other => Color::rgb(0.286, 0.314, 0.341),
    }
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

    pub fn clear(&mut self) {
        self.items.clear();
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
    fn files_are_sorted_into_categories_by_extension_whatever_the_case() {
        assert_eq!(category(Path::new("brief.PDF"), false), Category::Pdf);
        assert_eq!(category(Path::new("budget.xlsx"), false), Category::Spreadsheet);
        assert_eq!(category(Path::new("photo.JpEg"), false), Category::Image);
        assert_eq!(category(Path::new("noextension"), false), Category::Other);
        assert_eq!(category(Path::new("anything.pdf"), true), Category::Folder);
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
    fn removing_takes_one_file_off_and_clearing_takes_all() {
        let directory = temporary_directory("remove");
        let mut shelf = Shelf::default();
        shelf.add([file(&directory, "a.txt"), file(&directory, "b.txt")]);
        assert_eq!(shelf.remove(0), Some(directory.join("a.txt")));
        assert_eq!(shelf.remove(5), None);
        assert_eq!(shelf.len(), 1);
        shelf.clear();
        assert_eq!(shelf.len(), 0);
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
