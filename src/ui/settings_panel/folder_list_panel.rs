use crate::ui::settings_panel::HashListPanel;
use crate::util::{FONT_SIZE, PathBufToString};
use eframe::emath::Vec2;
use egui::{Button, Label, Sense};
use egui_extras::{Column, TableBuilder};
use rfd::FileDialog;
use std::path::PathBuf;

#[derive(Debug)]
pub(super) struct SearchFolderPanel<'a> {
    id_salt: &'a str,
    title: Option<&'a str>,
    data: HashListPanel<'a, PathBuf>,
}

impl<'a> SearchFolderPanel<'a> {
    pub(super) const fn with_title(
        id: &'a str,
        title: &'a str,
        data: HashListPanel<'a, PathBuf>,
    ) -> Self {
        Self {
            id_salt: id,
            title: Some(title),
            data,
        }
    }

    pub(super) fn show(mut self, ui: &mut egui::Ui) -> bool {
        ui.vertical(|ui| {
            if let Some(title) = self.title {
                ui.heading(title);
                ui.separator();
            }
            ui.horizontal(|ui| {
                if ui
                    .add_sized(Vec2::new(FONT_SIZE, FONT_SIZE), Button::new("+"))
                    .clicked()
                {
                    let files = FileDialog::new().pick_folders();
                    files
                        .into_iter()
                        .flatten()
                        .for_each(|path| self.data.push(path));
                }
                if ui
                    .add_sized(Vec2::new(FONT_SIZE, FONT_SIZE), Button::new("-"))
                    .clicked()
                {
                    self.data.remove_selection();
                }
            });
            TableBuilder::new(ui)
                .id_salt(self.id_salt)
                .striped(true)
                .sense(Sense::click())
                .column(Column::remainder())
                .body(|body| {
                    body.rows(FONT_SIZE, self.data.len(), |mut row| {
                        let folder = &self.data[row.index()];
                        row.set_selected(*self.data.selection == Some(row.index()));
                        if row
                            .col(|ui| {
                                ui.add(Label::new(folder.absolute_path()).selectable(false));
                            })
                            .1
                            .clicked()
                        {
                            self.toggle_selection(row.index());
                        }
                    });
                });
        });
        self.data.dirty
    }

    fn toggle_selection(&mut self, clicked_row: usize) {
        if *self.data.selection == Some(clicked_row) {
            *self.data.selection = None;
        } else {
            *self.data.selection = Some(clicked_row);
        }
    }
}
