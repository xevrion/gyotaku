//! Review selected image names before changing any files.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, ClickEvent, Context, CursorStyle, Entity, ListAlignment, ListState,
    PathPromptOptions, Window, div, list, prelude::*, px,
};
use gyotaku_core::naming::{files_have_same_content, plan_rename, rename_no_overwrite};
use gyotaku_core::{Config, Index, NameOptions, PlannedRename};
use gyotaku_ocr::Ocr;

use super::{Gyotaku, Page};
use crate::input::{Changed, TextInput};
use crate::platform;
use crate::theme::Theme;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceMode {
    Files,
    Folder,
}

pub(super) struct Naming {
    mode: SourceMode,
    folder: Option<PathBuf>,
    files: Vec<PathBuf>,
    include_subfolders: bool,
    rows: Vec<NameRow>,
    list: ListState,
    loading: bool,
    applying: bool,
    message: Option<String>,
    generation: u64,
}

struct NameRow {
    source: PathBuf,
    target: Entity<TextInput>,
    selected: bool,
    status: Option<String>,
    done: bool,
}

struct PreviewRow {
    source: PathBuf,
    plan: Option<PlannedRename>,
    status: Option<String>,
}

impl Naming {
    fn new() -> Self {
        Self {
            mode: SourceMode::Files,
            folder: None,
            files: Vec::new(),
            include_subfolders: false,
            rows: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, px(68.)),
            loading: false,
            applying: false,
            message: None,
            generation: 0,
        }
    }
}

fn supported(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["png", "jpg", "jpeg", "webp"]
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

fn scan_folder(folder: &Path, recursive: bool, paths: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let path = entry.path();
        if kind.is_file() && supported(&path) {
            paths.push(path);
        } else if recursive && kind.is_dir() {
            scan_folder(&path, true, paths)?;
        }
    }
    Ok(())
}

fn preview(paths: Vec<PathBuf>) -> Vec<PreviewRow> {
    let mut used = HashSet::new();
    let options = NameOptions::load(None);
    let scripts = Config::load_or_default().scripts();
    let index = Index::open_default().ok();
    let mut ocr: Option<Ocr> = None;
    paths
        .into_iter()
        .map(|source| {
            let lines = (|| {
                let metadata = source.metadata().map_err(|e| e.to_string())?;
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |duration| duration.as_secs() as i64);
                if let Some(index) = &index
                    && let Ok(Some(lines)) = index.lines_for_path(&source, modified)
                {
                    return Ok(lines);
                }
                let (width, height) =
                    image::image_dimensions(&source).map_err(|e| e.to_string())?;
                if u64::from(width) * u64::from(height) > 100_000_000 {
                    return Err("image is too large to read safely".to_string());
                }
                let image = image::open(&source).map_err(|e| e.to_string())?.into_rgb8();
                if ocr.is_none() {
                    ocr = Some(
                        Ocr::new(gyotaku_core::default_threads(), &scripts)
                            .map_err(|e| e.to_string())?,
                    );
                }
                ocr.as_mut()
                    .unwrap()
                    .read(&image)
                    .map_err(|e| e.to_string())
            })();
            match lines {
                Ok(lines) => match plan_rename(&source, &lines, &options, &mut used) {
                    Some(plan) => {
                        let status = if plan.target_path == source {
                            Some("Already named".into())
                        } else if plan.target_path.exists()
                            && files_have_same_content(&source, &plan.target_path)
                        {
                            Some("Already present".into())
                        } else {
                            None
                        };
                        PreviewRow {
                            source,
                            plan: Some(plan),
                            status,
                        }
                    }
                    None => PreviewRow {
                        source,
                        plan: None,
                        status: Some("No title found".into()),
                    },
                },
                Err(error) => PreviewRow {
                    source,
                    plan: None,
                    status: Some(error),
                },
            }
        })
        .collect()
}

fn valid_name(source: &Path, name: &str) -> bool {
    if name.is_empty()
        || name.len() > 255
        || name.trim() != name
        || name.ends_with('.')
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || Path::new(name).file_name().is_none_or(|file| file != name)
    {
        return false;
    }
    let candidate = Path::new(name);
    let same_extension = candidate
        .extension()
        .and_then(|s| s.to_str())
        .zip(source.extension().and_then(|s| s.to_str()))
        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b));
    let stem = candidate.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let base = stem.split('.').next().unwrap_or("").to_ascii_lowercase();
    let reserved = matches!(base.as_str(), "con" | "prn" | "aux" | "nul")
        || (base.len() == 4
            && (base.starts_with("com") || base.starts_with("lpt"))
            && base.as_bytes()[3].is_ascii_digit());
    same_extension && !stem.is_empty() && !reserved
}

impl Gyotaku {
    pub(super) fn open_naming(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.page = Page::Naming(Naming::new());
        window.focus(&self.panel_focus, cx);
        cx.notify();
    }

    fn naming_mode(&mut self, mode: SourceMode, cx: &mut Context<Self>) {
        if let Page::Naming(naming) = &mut self.page {
            naming.mode = mode;
        }
        self.refresh_naming(cx);
    }

    fn choose_naming_source(&mut self, folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: !folder,
            directories: folder,
            multiple: !folder,
            prompt: Some(
                if folder {
                    "Choose folder"
                } else {
                    "Choose images"
                }
                .into(),
            ),
        });
        let step_aside = self.floating && crate::is_resident(cx);
        if step_aside {
            window.remove_window();
        }
        platform::before_picker(cx);
        cx.spawn(async move |this, cx| {
            let result = picked.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(Ok(Some(paths))) => {
                    if let Page::Naming(naming) = &mut this.page {
                        if folder {
                            naming.folder = paths.into_iter().next();
                            naming.mode = SourceMode::Folder;
                        } else {
                            naming.files = paths;
                            naming.mode = SourceMode::Files;
                        }
                        this.refresh_naming(cx);
                    }
                }
                Ok(Ok(None)) => {}
                _ => this.flash("couldn't open the image picker", cx),
            });
            if step_aside {
                cx.update(crate::summon);
            }
        })
        .detach();
    }

    fn refresh_naming(&mut self, cx: &mut Context<Self>) {
        let Page::Naming(naming) = &mut self.page else {
            return;
        };
        naming.generation += 1;
        let generation = naming.generation;
        naming.rows.clear();
        naming.list.reset(0);
        naming.message = None;
        let source = match naming.mode {
            SourceMode::Files => Some((naming.files.clone(), None)),
            SourceMode::Folder => naming
                .folder
                .clone()
                .map(|folder| (Vec::new(), Some((folder, naming.include_subfolders)))),
        };
        let Some((files, folder)) = source else {
            naming.loading = false;
            cx.notify();
            return;
        };
        naming.loading = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut paths = files;
                    if let Some((folder, recursive)) = folder {
                        scan_folder(&folder, recursive, &mut paths)
                            .map_err(|e| format!("couldn't read {}: {e}", folder.display()))?;
                    }
                    paths.retain(|path| path.is_file() && supported(path));
                    paths.sort_unstable();
                    paths.dedup();
                    Ok::<_, String>(preview(paths))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let Page::Naming(naming) = &mut this.page else {
                    return;
                };
                if naming.generation != generation {
                    return;
                }
                naming.loading = false;
                match result {
                    Ok(previews) => {
                        for row in previews {
                            let proposed = row
                                .plan
                                .as_ref()
                                .map_or("", |plan| plan.recommended_filename.as_str());
                            let input = cx.new(|cx| TextInput::new("Proposed filename".into(), cx));
                            input.update(cx, |input, cx| {
                                input.content = proposed.to_string().into();
                                cx.notify();
                            });
                            cx.subscribe(&input, |this, _, _: &Changed, cx| {
                                if matches!(this.page, Page::Naming(_)) {
                                    cx.notify();
                                }
                            })
                            .detach();
                            naming.rows.push(NameRow {
                                source: row.source,
                                target: input,
                                selected: row.plan.is_some() && row.status.is_none(),
                                status: row.status,
                                done: false,
                            });
                        }
                        naming.list.reset(naming.rows.len());
                    }
                    Err(error) => naming.message = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn rename_naming_selection(&mut self, cx: &mut Context<Self>) {
        let Page::Naming(naming) = &mut self.page else {
            return;
        };
        if naming.loading || naming.applying {
            return;
        }
        let mut targets = HashSet::new();
        let mut changes = Vec::new();
        for (index, row) in naming.rows.iter().enumerate() {
            if !row.selected || row.done || row.status.is_some() {
                continue;
            }
            let name = row.target.read(cx).content.to_string();
            if !valid_name(&row.source, &name) {
                naming.message = Some(format!("Invalid filename: {name}"));
                cx.notify();
                return;
            }
            let target = row.source.with_file_name(&name);
            if target == row.source {
                continue;
            }
            if !targets.insert(target.to_string_lossy().to_ascii_lowercase()) || target.exists() {
                naming.message = Some(format!("Target name already exists: {name}"));
                cx.notify();
                return;
            }
            changes.push((index, row.source.clone(), target));
        }
        if changes.is_empty() {
            naming.message = Some("No new names selected".into());
            cx.notify();
            return;
        }
        naming.applying = true;
        naming.message = None;
        let generation = naming.generation;
        cx.spawn(async move |this, cx| {
            let results = cx
                .background_executor()
                .spawn(async move {
                    changes
                        .into_iter()
                        .map(|(index, source, target)| {
                            let result =
                                rename_no_overwrite(&source, &target).map_err(|e| e.to_string());
                            (index, source, target, result)
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let Page::Naming(naming) = &mut this.page else {
                    return;
                };
                if naming.generation != generation {
                    return;
                }
                naming.applying = false;
                let mut changed = 0;
                let mut failed = 0;
                for (index, source, target, result) in results {
                    match result {
                        Ok(()) => {
                            changed += 1;
                            let _ = this.index.rename(&source, &target);
                            if let Some(row) = naming.rows.get_mut(index) {
                                row.source = target;
                                row.selected = false;
                                row.done = true;
                            }
                        }
                        Err(error) => {
                            failed += 1;
                            if let Some(row) = naming.rows.get_mut(index) {
                                row.status = Some(error);
                            }
                        }
                    }
                }
                naming.message = Some(format!(
                    "Renamed {changed} images{}",
                    if failed > 0 {
                        format!(", {failed} failed")
                    } else {
                        String::new()
                    }
                ));
                this.search(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn render_naming(
        &mut self,
        theme: Theme,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Page::Naming(naming) = &self.page else {
            return div().into_any_element();
        };
        let folder_mode = naming.mode == SourceMode::Folder;
        let include_subfolders = naming.include_subfolders;
        let folder = naming
            .folder
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "No folder chosen".into());
        let files = naming.files.len();
        let total = naming.rows.len();
        let selected = naming
            .rows
            .iter()
            .filter(|row| {
                row.selected
                    && !row.done
                    && row.status.is_none()
                    && !row.target.read(cx).content.is_empty()
            })
            .count();
        let loading = naming.loading;
        let applying = naming.applying;
        let message = naming.message.clone();
        let list_state = naming.list.clone();
        let tab = |id, label, active, mode| {
            div()
                .id(id)
                .px_3()
                .py_2()
                .rounded(px(6.))
                .bg(if active { theme.keycap } else { theme.panel })
                .cursor(CursorStyle::PointingHand)
                .child(label)
                .on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.naming_mode(mode, cx)),
                )
        };
        div().size_full().flex().flex_col()
            .child(div().h(px(58.)).px_4().flex().items_center().justify_between().border_b_1().border_color(theme.hairline)
                .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Rename images"))
                .child(div().id("naming-back").cursor(CursorStyle::PointingHand).child("Back to search")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.leave_panel(window, cx)))))
            .child(div().flex_1().min_h_0().flex().flex_col().px_4().pt_4()
                .child(div().text_sm().text_color(theme.muted).child("Review the proposed names, then rename the selected files in place. No copies are made."))
                .child(div().flex().gap_2().mt_4()
                    .child(tab("naming-files", "Specific images", !folder_mode, SourceMode::Files))
                    .child(tab("naming-folder", "Folder", folder_mode, SourceMode::Folder)))
                .child(div().flex().items_center().justify_between().mt_4().p_3().border_1().border_color(theme.hairline).rounded(px(8.))
                    .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().child(if folder_mode { folder } else { format!("{files} specific images") }))
                    .child(div().id("naming-choose").ml_3().px_3().py_2().rounded(px(6.)).bg(theme.keycap).cursor(CursorStyle::PointingHand)
                        .child(if folder_mode { "Choose folder" } else { "Choose images" })
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.choose_naming_source(folder_mode, window, cx)))))
                .when(folder_mode, |panel| panel.child(div().id("naming-recursive").flex().items_center().gap_2().py_3().cursor(CursorStyle::PointingHand)
                    .child(div().w(px(18.)).h(px(18.)).border_1().border_color(theme.text).bg(if include_subfolders { theme.text } else { theme.panel }))
                    .child("Include subfolders")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Page::Naming(naming) = &mut this.page { naming.include_subfolders = !naming.include_subfolders; }
                        this.refresh_naming(cx);
                    }))))
                .child(div().flex().items_center().justify_between().mt_3().pb_2().border_b_1().border_color(theme.hairline)
                    .child(div().text_lg().child("Name preview"))
                    .child(div().text_sm().text_color(theme.muted).child(if loading { "Reading images...".into() } else { format!("{total} shown, {selected} selected") })))
                .child(list(list_state, cx.processor(Self::render_naming_row)).flex_1().min_h_0())
                .child(div().py_2().text_sm().text_color(theme.muted).child("Names that already exist are blocked before renaming. Click a row or its checkbox to change the selection."))
                .when_some(message, |panel, message| panel.child(div().py_2().text_sm().child(message)))
                .child(div().flex().items_center().justify_between().gap_3().py_3().border_t_1().border_color(theme.hairline)
                    .child(div().text_sm().text_color(theme.muted).child("In-place rename. File contents stay where they are."))
                    .child(div().id("naming-apply").px_4().py_2().rounded(px(7.)).bg(if selected > 0 && !loading && !applying { theme.text } else { theme.keycap })
                        .text_color(if selected > 0 && !loading && !applying { theme.panel } else { theme.muted })
                        .cursor(CursorStyle::PointingHand)
                        .child(if applying { "Renaming...".into() } else { format!("Rename {selected} images") })
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.rename_naming_selection(cx))))))
            .into_any_element()
    }

    fn render_naming_row(
        &mut self,
        index: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Page::Naming(naming) = &self.page else {
            return div().into_any_element();
        };
        let Some(row) = naming.rows.get(index) else {
            return div().into_any_element();
        };
        let theme = *cx.global::<Theme>();
        let selected = row.selected;
        let source = row
            .source
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let parent = row
            .source
            .parent()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let status = if row.done {
            "Renamed".to_string()
        } else {
            row.status.clone().unwrap_or_else(|| "Ready".into())
        };
        let target = row.target.clone();
        div()
            .id(("naming-row", index))
            .h(px(68.))
            .px_3()
            .flex()
            .items_center()
            .gap_3()
            .border_b_1()
            .border_color(theme.hairline)
            .cursor(CursorStyle::PointingHand)
            .hover(move |style| style.bg(theme.hover_wash))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Page::Naming(naming) = &mut this.page
                    && let Some(row) = naming.rows.get_mut(index)
                    && row.status.is_none()
                    && !row.done
                {
                    row.selected = !row.selected;
                    cx.notify();
                }
            }))
            .child(
                div()
                    .w(px(18.))
                    .h(px(18.))
                    .flex_none()
                    .border_1()
                    .border_color(theme.text)
                    .bg(if selected { theme.text } else { theme.panel }),
            )
            .child(
                div()
                    .w(px(230.))
                    .min_w_0()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(div().whitespace_nowrap().text_ellipsis().child(source))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted)
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(parent),
                    ),
            )
            .child(
                div()
                    .id(("naming-target", index))
                    .flex_1()
                    .min_w_0()
                    .px_2()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(target),
            )
            .child(
                div()
                    .w(px(96.))
                    .text_xs()
                    .text_color(theme.muted)
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(status),
            )
            .into_any_element()
    }
}
