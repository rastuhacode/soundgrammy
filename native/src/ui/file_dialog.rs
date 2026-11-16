//! File selection returns ordinary local files to the playlist service. Android
//! content URIs are copied through ContentResolver rather than treated as paths.
#[cfg(target_os = "android")]
use crate::error::AppError;
use crate::error::AppResult;
use std::path::PathBuf;
pub struct SaveTarget {
    pub path: PathBuf,
    #[cfg(target_os = "android")]
    uri: Option<String>,
}
impl SaveTarget {
    pub fn commit(self) -> AppResult<()> {
        #[cfg(target_os = "android")]
        if let Some(uri) = self.uri {
            crate::android_keyring::copy_document(&uri, &self.path, true).map_err(AppError::msg)?;
            let _ = std::fs::remove_file(self.path);
        }
        Ok(())
    }
}
pub fn open_json() -> AppResult<Option<PathBuf>> {
    #[cfg(not(target_os = "android"))]
    {
        Ok(rfd::FileDialog::new()
            .add_filter("SoundGrammy playlist", &["json"])
            .pick_file())
    }
    #[cfg(target_os = "android")]
    {
        use gpui_mobile::packages::file_selector::*;
        let selected = open_file(&OpenFileOptions {
            accept_type_groups: vec![TypeGroup {
                label: "SoundGrammy playlist".into(),
                extensions: vec!["json".into()],
                mime_types: vec!["application/json".into()],
                ..Default::default()
            }],
            ..Default::default()
        })
        .map_err(AppError::msg)?;
        selected
            .map(|selected| {
                if !selected.path.starts_with("content:") {
                    return Ok(PathBuf::from(selected.path));
                }
                let path = gpui_mobile::packages::path_provider::cache_directory()
                    .map_err(AppError::msg)?
                    .join(format!("playlist-import-{}.json", crate::time::now_ms()));
                crate::android_keyring::copy_document(&selected.path, &path, false)
                    .map_err(AppError::msg)?;
                Ok(path)
            })
            .transpose()
    }
}
pub fn save_json(name: String) -> AppResult<Option<SaveTarget>> {
    #[cfg(not(target_os = "android"))]
    {
        Ok(rfd::FileDialog::new()
            .add_filter("SoundGrammy playlist", &["json"])
            .set_file_name(format!("{name}.soundgrammy.json"))
            .save_file()
            .map(|path| SaveTarget { path }))
    }
    #[cfg(target_os = "android")]
    {
        use gpui_mobile::packages::file_selector::*;
        let selected = get_save_path(&SaveFileOptions {
            suggested_name: Some(format!("{name}.soundgrammy.json")),
            accept_type_groups: vec![TypeGroup {
                label: "SoundGrammy playlist".into(),
                extensions: vec!["json".into()],
                mime_types: vec!["application/json".into()],
                ..Default::default()
            }],
            ..Default::default()
        })
        .map_err(AppError::msg)?;
        selected
            .map(|uri| {
                let path = gpui_mobile::packages::path_provider::cache_directory()
                    .map_err(AppError::msg)?
                    .join(format!("playlist-export-{}.json", crate::time::now_ms()));
                Ok(SaveTarget {
                    path,
                    uri: Some(uri),
                })
            })
            .transpose()
    }
}
