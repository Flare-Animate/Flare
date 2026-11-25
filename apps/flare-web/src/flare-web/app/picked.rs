//! The files the person picked, waiting since the picker answered, handed to the workspace at
//! the start of a frame: each kind to what reads it.

use super::{Picked, WebApp, is_flare};

impl WebApp {
    /// Hands every file picked since the last frame to the workspace.
    pub(super) fn hand_over_picked(&mut self) {
        let picked: Vec<Picked> = self.picked.borrow_mut().drain(..).collect();
        for file in picked {
            self.last_name = Some(file.name.clone());
            if is_flare(&file.name) {
                self.open_flare(&file.name, &file.bytes);
            } else {
                self.workspace.import_project_bytes(&file.name, file.bytes);
            }
        }
        let sounds: Vec<Picked> = self.picked_sounds.borrow_mut().drain(..).collect();
        for sound in sounds {
            self.workspace.import_sound_bytes(&sound.name, sound.bytes);
        }
        for (inbox, to_stage) in [
            (&self.picked_to_stage, true),
            (&self.picked_to_library, false),
        ] {
            let files: Vec<Picked> = inbox.borrow_mut().drain(..).collect();
            for file in files {
                self.workspace
                    .import_into_document_bytes(&file.name, file.bytes, to_stage);
            }
        }
        let shortcuts: Vec<Picked> = self.picked_shortcuts.borrow_mut().drain(..).collect();
        for file in shortcuts {
            self.workspace.import_animate_shortcuts(&file.bytes);
        }
        let references: Vec<Picked> = self.picked_references.borrow_mut().drain(..).collect();
        for file in references {
            self.workspace
                .import_mannequin_reference_bytes(&file.name, file.bytes);
        }
        let brushes: Vec<Picked> = self.picked_brushes.borrow_mut().drain(..).collect();
        for file in brushes {
            self.workspace
                .import_brush_profiles(&file.name, &file.bytes);
        }
        let shaders: Vec<Picked> = self.picked_shaders.borrow_mut().drain(..).collect();
        for file in shaders {
            let name = file.name.strip_suffix(".json").unwrap_or(&file.name);
            self.workspace.import_shader_graph(name, &file.bytes);
        }
        let frames: Vec<_> = self.picked_frames.borrow_mut().drain(..).collect();
        for files in frames {
            self.workspace
                .add_stop_motion_pictures(flare_animate::decode_stop_motion_pictures(files));
        }
        let sequences: Vec<_> = self.picked_sequences.borrow_mut().drain(..).collect();
        for files in sequences {
            self.workspace.import_image_sequence(files);
        }
        let movies: Vec<Picked> = self.picked_movies.borrow_mut().drain(..).collect();
        for file in movies {
            let movie = flare_as3::Movie::read(&file.bytes);
            self.workspace.show_movie_scripts(file.name, movie);
        }
    }
}
