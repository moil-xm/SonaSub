mod commands;
mod devices;
mod events;
mod lifecycle;
mod manager;
mod runtime;
pub mod source;
pub mod store;
pub mod subtitles;
pub mod types;

pub use commands::{
    add_record_device, create_record, get_record, list_record_devices, list_record_subtitles,
    list_records, pause_record, remove_record_device, resume_record, start_record, stop_record,
    update_record,
};
pub use manager::RecordManager;

#[doc(hidden)]
pub(crate) use commands::{
    __cmd__add_record_device, __cmd__create_record, __cmd__get_record, __cmd__list_record_devices,
    __cmd__list_record_subtitles, __cmd__list_records, __cmd__pause_record,
    __cmd__remove_record_device, __cmd__resume_record, __cmd__start_record, __cmd__stop_record,
    __cmd__update_record, __tauri_command_name_add_record_device,
    __tauri_command_name_create_record, __tauri_command_name_get_record,
    __tauri_command_name_list_record_devices, __tauri_command_name_list_record_subtitles,
    __tauri_command_name_list_records, __tauri_command_name_pause_record,
    __tauri_command_name_remove_record_device, __tauri_command_name_resume_record,
    __tauri_command_name_start_record, __tauri_command_name_stop_record,
    __tauri_command_name_update_record,
};
