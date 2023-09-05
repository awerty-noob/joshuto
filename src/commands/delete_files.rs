use std::path;

use termion::event::Key;

use tokio::sync::mpsc;

use crate::context::AppContext;
use crate::error::{AppResult, JoshutoError, JoshutoErrorKind};
use crate::history::DirectoryHistory;
use crate::io::{FileOperation, FileOperationOptions, IoWorkerThread};
use crate::ui::widgets::TuiPrompt;
use crate::ui::AppBackend;

async fn prompt(context: &mut AppContext, backend: &mut AppBackend, paths_len: usize) -> bool {
    let ch = {
        let prompt_str = format!("Delete {} files? (Y/n)", paths_len);
        let mut prompt = TuiPrompt::new(&prompt_str);
        prompt.get_key(backend, context).await
    };

    match ch {
        Key::Char('Y') | Key::Char('y') | Key::Char('\n') => {
            if paths_len > 1 {
                // prompt user again for deleting multiple files
                let ch2 = {
                    let prompt_str = "Are you sure? (y/N)";
                    let mut prompt = TuiPrompt::new(prompt_str);
                    prompt.get_key(backend, context).await
                };
                ch2 == Key::Char('y')
            } else {
                true
            }
        }
        _ => false,
    }
}

async fn delete_files(
    context: &mut AppContext,
    paths: Vec<path::PathBuf>,
    background: bool,
    permanently: bool,
) -> AppResult<()> {
    let file_op = FileOperation::Delete;
    let options = FileOperationOptions {
        overwrite: false,
        skip_exist: false,
        permanently: !context.config_ref().use_trash || permanently,
    };

    let dest = path::PathBuf::new();
    let worker_thread = IoWorkerThread::new(file_op, paths.clone(), dest, options);
    if background {
        context
            .worker_context_mut()
            .push_worker(worker_thread)
            .await;
    } else {
        let (wtx, _) = mpsc::channel(1024);
        worker_thread.start(wtx).await?;
    }

    let history = context.tab_context_mut().curr_tab_mut().history_mut();
    for path in paths.iter().filter(|p| p.is_dir()) {
        history.remove(path);
    }

    Ok(())
}

pub async fn delete_selected_files(
    context: &mut AppContext,
    backend: &mut AppBackend,
    background: bool,
    permanently: bool,
    noconfirm: bool,
) -> AppResult {
    let paths = context
        .tab_context_ref()
        .curr_tab_ref()
        .curr_list_ref()
        .map(|s| s.get_selected_paths())
        .unwrap_or_default();

    let paths_len = paths.len();
    if paths_len == 0 {
        let err = JoshutoError::new(
            JoshutoErrorKind::InvalidParameters,
            "no files selected".to_string(),
        );
        return Err(err);
    }

    if noconfirm || prompt(context, backend, paths_len).await {
        delete_files(context, paths, background, permanently).await?;
    }

    let curr_tab = context.tab_context_ref().curr_tab_ref();
    let options = context.config_ref().display_options_ref().clone();
    let curr_path = curr_tab.cwd().to_path_buf();
    for (_, tab) in context.tab_context_mut().iter_mut() {
        let tab_options = tab.option_ref().clone();
        tab.history_mut()
            .reload(&curr_path, &options, &tab_options)?;
    }
    Ok(())
}
