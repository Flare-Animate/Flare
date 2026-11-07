#![allow(clippy::expect_used)]
//! A cast folder lists levels by id only; those lines are references, not declarations.

use flare_toonz::parse_scene;

#[test]
fn a_folder_that_lists_declared_levels_by_id_is_not_a_duplicate() {
    let scene = parse_scene(
        r#"<tnz version="71.0"><properties/>
           <levelSet>
             <levels>
               <level id='1' name='A' type='vector'><path>+drawings/A.pli</path></level>
             </levels>
             <folder name="Cast">
               <levels><level id='1'/></levels>
             </folder>
           </levelSet></tnz>"#,
    )
    .expect("a reference by id is not a second declaration");
    assert_eq!(scene.levels.len(), 1);
    assert_eq!(scene.levels[0].name, "A");
}

#[test]
fn a_second_full_declaration_of_the_same_id_is_still_refused() {
    let err = parse_scene(
        r#"<tnz version="71.0"><properties/>
           <levelSet>
             <levels>
               <level id='1' name='A' type='vector'/>
               <level id='1' name='B' type='vector'/>
             </levels>
           </levelSet></tnz>"#,
    )
    .expect_err("two declarations of one id");
    assert!(err.to_string().contains("declared twice"), "{err}");
}
