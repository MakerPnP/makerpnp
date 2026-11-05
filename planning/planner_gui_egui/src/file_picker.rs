use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tracing::debug;

/// The files of a kind a file dialog shows, by their extensions, without the dot.
#[derive(Debug, Clone, PartialEq)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

/// A file to write, which may not exist yet, as asked for with a file dialog.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SaveFile {
    /// The folder the dialog starts in.
    pub directory: Option<PathBuf>,
    /// The file's name, suggested.
    pub file_name: String,
    pub filters: Vec<FileFilter>,
}

/// The platform's file dialogs, an interface so that tests can answer them without a user.
///
/// All the methods block until there is an answer, `None` means the user cancelled. A dialog given filters shows only
/// the files they match, or all if there are none.
pub trait FileDialogs: Debug + Send + Sync {
    fn pick_file(&self, filters: &[FileFilter]) -> Option<PathBuf>;
    fn pick_files(&self, filters: &[FileFilter]) -> Option<Vec<PathBuf>>;
    fn pick_folder(&self) -> Option<PathBuf>;
    fn save_file(&self, save_file: &SaveFile) -> Option<PathBuf>;
}

/// The operating system's file dialogs.
#[derive(Debug)]
pub struct NativeFileDialogs;

impl NativeFileDialogs {
    fn dialog(filters: &[FileFilter]) -> rfd::FileDialog {
        filters
            .iter()
            .fold(rfd::FileDialog::new(), |dialog, filter| {
                dialog.add_filter(&filter.name, &filter.extensions)
            })
    }
}

impl FileDialogs for NativeFileDialogs {
    fn pick_file(&self, filters: &[FileFilter]) -> Option<PathBuf> {
        Self::dialog(filters).pick_file()
    }

    fn pick_files(&self, filters: &[FileFilter]) -> Option<Vec<PathBuf>> {
        Self::dialog(filters).pick_files()
    }

    fn pick_folder(&self) -> Option<PathBuf> {
        rfd::FileDialog::new().pick_folder()
    }

    fn save_file(&self, save_file: &SaveFile) -> Option<PathBuf> {
        let dialog = Self::dialog(&save_file.filters).set_file_name(&save_file.file_name);
        match save_file.directory.as_deref() {
            Some(directory) => dialog.set_directory(directory),
            None => dialog,
        }
        .save_file()
    }
}

/// The extensions of the filters, for the log.
fn extensions(filters: &[FileFilter]) -> Vec<&str> {
    filters
        .iter()
        .flat_map(|filter| {
            filter
                .extensions
                .iter()
                .map(String::as_str)
        })
        .collect()
}

/// A file picker
/// * prevents multiple concurrent pick operations
/// * provides an API convenient for UI usage (see `is_picking` and `picked`)
///
/// Currently only picks files, but the API could be expanded.
#[derive(Debug)]
pub struct Picker {
    state: PickerState,
    dialogs: Arc<dyn FileDialogs>,
}

impl Default for Picker {
    fn default() -> Self {
        Self::new(Arc::new(NativeFileDialogs))
    }
}

#[derive(Default, Debug)]
enum PickerState {
    #[default]
    Pending,
    // use a boolean to indicate of picking has completed
    Picking(Arc<Mutex<(bool, Option<Vec<PathBuf>>)>>),
}

#[derive(Clone, PartialEq)]
pub enum PickError {
    NotPicking,
    InProgress,
    Cancelled,
}

impl Picker {
    pub fn new(dialogs: Arc<dyn FileDialogs>) -> Self {
        Self {
            state: PickerState::default(),
            dialogs,
        }
    }

    pub fn is_picking(&self) -> bool {
        matches!(self.state, PickerState::Picking(_))
    }

    fn prepare(&mut self, some_fn: impl FnOnce(&dyn FileDialogs) -> Option<Vec<PathBuf>> + Send + 'static) {
        // initialise the boolean flag in the mutex to false, so that when the main thread continues it can see a
        // file has not been picked yet.  note that the mutex may not be locked until the picker thread starts to run
        // and lock it.
        let picker = Arc::new(Mutex::new((false, None)));
        self.state = PickerState::Picking(picker.clone());
        let dialogs = self.dialogs.clone();
        std::thread::Builder::new()
            .name("picker".to_owned())
            .spawn(move || {
                let mut guard = picker.lock().unwrap();
                *guard = (true, some_fn(dialogs.as_ref()));
            })
            .unwrap();
    }

    pub fn pick_file(&mut self) {
        self.pick_file_of(vec![]);
    }

    /// Picks a file the filters match.
    pub fn pick_file_of(&mut self, filters: Vec<FileFilter>) {
        debug!("Picking a file. extensions: {:?}", extensions(&filters));
        self.prepare(move |dialogs| {
            dialogs
                .pick_file(&filters)
                .map(|file| vec![file])
        });
    }

    pub fn pick_files(&mut self) {
        self.pick_files_of(vec![]);
    }

    /// Picks files the filters match.
    pub fn pick_files_of(&mut self, filters: Vec<FileFilter>) {
        debug!("Picking files. extensions: {:?}", extensions(&filters));
        self.prepare(move |dialogs| dialogs.pick_files(&filters));
    }

    /// Picks a file to write, which may not exist yet.
    pub fn save_file(&mut self, save_file: SaveFile) {
        debug!(
            "Picking a file to save. file_name: '{}', extensions: {:?}",
            save_file.file_name,
            extensions(&save_file.filters)
        );
        self.prepare(move |dialogs| {
            dialogs
                .save_file(&save_file)
                .map(|file| vec![file])
        });
    }

    pub fn pick_folder(&mut self) {
        self.prepare(|dialogs| {
            dialogs
                .pick_folder()
                .map(|folder| vec![folder])
        });
    }

    /// when picked, returns the picked paths, or an error indicating the reason
    ///
    /// See also [`Self::picked`]
    ///
    /// this method is designed to be very fast while the picker is not picking (pending)
    pub fn picked_multi(&mut self) -> Result<Vec<PathBuf>, PickError> {
        let mut was_picked = false;

        let result = match &mut self.state {
            PickerState::Picking(arc) => {
                if let Ok(mut guard) = arc.try_lock() {
                    match &mut *guard {
                        (true, picked) => {
                            was_picked = true;
                            match picked.take() {
                                Some(picked_path) => Ok(picked_path),
                                None => Err(PickError::Cancelled),
                            }
                        }
                        // arc not locked, but not picked yet either
                        (false, _) => Err(PickError::InProgress),
                    }
                } else {
                    // not picked yet, arc locked by thread
                    Err(PickError::InProgress)
                }
            }
            PickerState::Pending =>
            // not picked
            {
                Err(PickError::NotPicking)
            }
        };

        if was_picked {
            // this causes the arc and mutex to be dropped, ready for the next pick.
            self.state = PickerState::Pending;
        }

        result
    }

    /// when picked, returns the first picked path, or an error indicating the reason
    ///
    /// use when picking a single file or folder, otherwise use [`Self::picked_multi`]
    pub fn picked(&mut self) -> Result<PathBuf, PickError> {
        self.picked_multi()
            .map(|paths| paths.first().unwrap().clone())
    }
}
