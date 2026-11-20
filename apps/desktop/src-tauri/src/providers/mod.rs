pub(crate) mod commands;
mod remote;
mod store;

pub use commands::{
    fetch_provider_models, list_providers_command, save_provider_command, test_provider_command,
};
pub use remote::{fetch_models, test_provider};
pub use store::{list_providers, save_provider, ProviderConfig, ProviderIcon, ProviderView};

#[doc(hidden)]
pub(crate) use commands::{
    __cmd__fetch_provider_models, __cmd__list_providers_command, __cmd__save_provider_command,
    __cmd__test_provider_command, __tauri_command_name_fetch_provider_models,
    __tauri_command_name_list_providers_command, __tauri_command_name_save_provider_command,
    __tauri_command_name_test_provider_command,
};
