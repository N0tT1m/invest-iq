pub mod core;
pub mod agent;
pub mod risk;
pub mod market;
pub mod data;
pub mod trading;
pub mod tax;
pub mod advanced;
pub mod system;

use serenity::all::{CommandDataOption, CommandDataOptionValue};

/// Extract sub-options from a SubCommand or SubCommandGroup option.
pub fn get_sub_options(opt: &CommandDataOption) -> &[CommandDataOption] {
    match &opt.value {
        CommandDataOptionValue::SubCommand(opts) => opts,
        CommandDataOptionValue::SubCommandGroup(opts) => opts,
        _ => &[],
    }
}

/// Extract a string value from a subcommand's options by name.
pub fn get_string_opt(subcommand: &CommandDataOption, name: &str) -> Option<String> {
    for opt in get_sub_options(subcommand) {
        if opt.name == name {
            if let CommandDataOptionValue::String(s) = &opt.value {
                return Some(s.clone());
            }
        }
    }
    None
}

/// Extract an integer value from a subcommand's options by name.
pub fn get_int_opt(subcommand: &CommandDataOption, name: &str) -> Option<i64> {
    for opt in get_sub_options(subcommand) {
        if opt.name == name {
            if let CommandDataOptionValue::Integer(v) = &opt.value {
                return Some(*v);
            }
        }
    }
    None
}

/// Resolve the leaf subcommand and its options from a SubCommandGroup.
/// For `/iq agent pending`, the top-level option is the SubCommandGroup "agent",
/// and within it is the SubCommand "pending". This function returns ("pending", &options).
pub fn resolve_subcommand(group_opt: &CommandDataOption) -> Option<(&str, &CommandDataOption)> {
    let children = get_sub_options(group_opt);
    for child in children {
        if matches!(child.value, CommandDataOptionValue::SubCommand(_)) {
            return Some((child.name.as_str(), child));
        }
    }
    None
}
