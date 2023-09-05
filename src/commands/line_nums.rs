use crate::config::option::LineNumberStyle;
use crate::context::AppContext;
use crate::error::AppResult;

use super::reload;

pub fn switch_line_numbering(context: &mut AppContext, style: LineNumberStyle) -> AppResult {
    context
        .config_mut()
        .display_options_mut()
        .set_line_nums(style);
    reload::reload_dirlist(context)
}
