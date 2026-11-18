mod capabilities;
mod commands;
mod install;
mod paths;
mod registry;
mod status;
mod transfer;

pub use crate::translation::detect_translation_source;
pub use commands::{
    download_model, get_model_download_settings, import_model_files, list_model_installs,
    set_model_download_directory,
};
pub use install::{ModelInstallFile, ModelInstallGroup};
pub use paths::ModelPaths;
pub use registry::{
    language_label, mode_label, resolve_asr_mode, resolve_translation_mode, translation_files,
    verify_files, AsrModelKind, AsrModelSpec, ModelFile, TranslationModelSpec, ASR_MODELS,
    ASR_SEGMENTED_ID, ASR_STREAMING_ID, BERGAMOT_ENGINE_ID, DIARIZATION_FILES, LANGUAGE_LABEL_ZH,
    SPEAKER_FILES, SUPPORTED_LANGUAGES, TRANSLATION_MODELS, VAD_FILES,
};
pub use status::{
    AsrModelCapability, AudioCapabilities, Capabilities, LanguageOption, ModeCapability,
    ModelDownloadSettings, ModelFileStatus, ModelStatus, TranslationModelCapability,
};

#[doc(hidden)]
pub(crate) use commands::{
    __cmd__download_model, __cmd__get_model_download_settings, __cmd__import_model_files,
    __cmd__list_model_installs, __cmd__set_model_download_directory,
    __tauri_command_name_download_model, __tauri_command_name_get_model_download_settings,
    __tauri_command_name_import_model_files, __tauri_command_name_list_model_installs,
    __tauri_command_name_set_model_download_directory,
};
