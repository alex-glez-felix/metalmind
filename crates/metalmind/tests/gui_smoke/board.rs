//! La pizarra en la app real: se crea, se abre, sus teclas no tocan el archivo
//! y una tarjeta abre su nota.

use crate::support::Smoke;
use crate::support::wait::{self, TIMEOUT};

const BOARD: &str = r#"{
	"nodes":[
		{"id":"c","type":"file","x":0,"y":0,"width":200,"height":80,"file":"nota.md"}
	],
	"edges":[]
}
"#;

#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn board_crea_una_pizarra_vacia_y_la_abre() {
    let smoke = Smoke::new();
    smoke.write("a.md", "uno\n");
    let mut metalmind = smoke.launch(&["a.md"]);
    let window = metalmind.window();
    window.type_keys(":board idea<CR>");
    window.wait_title("de la pizarra", |title| title.starts_with("idea.canvas"));
    window.type_keys(":q!<CR>");
    assert!(metalmind.wait_exit().success());
    assert_eq!(
        smoke.read("idea.canvas"),
        "{\n\t\"nodes\":[],\n\t\"edges\":[]\n}\n"
    );
}

/// En X11 `shift-0` llega como `)`, y al abrir se ve toda la pizarra a menos
/// del 100 %: la captura de lo que cambia dice si llegó.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn shift_y_un_numero_cambian_el_zoom() {
    let smoke = Smoke::new();
    let far = BOARD.replace(r#""width":200"#, r#""width":6000"#);
    smoke.write("idea.canvas", &far);
    let mut metalmind = smoke.launch(&["idea.canvas"]);
    let window = metalmind.window();
    window.wait_title("de la pizarra", |title| title.starts_with("idea.canvas"));
    let seen = window.pixels().2;
    window.type_keys(")");
    wait::until("el zoom al 100 %", TIMEOUT, || {
        if window.pixels().2 == seen {
            Err("igual".to_owned())
        } else {
            Ok(())
        }
    });
    window.type_keys("!");
    wait::until("la pizarra entera", TIMEOUT, || {
        if window.pixels().2 == seen {
            Ok(())
        } else {
            Err("distinta".to_owned())
        }
    });
    window.type_keys(":q!<CR>");
    assert!(metalmind.wait_exit().success());
}

#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn las_teclas_de_una_pizarra_no_tocan_el_archivo_y_enter_abre_la_nota() {
    let smoke = Smoke::new();
    smoke.write("nota.md", "la nota\n");
    smoke.write("idea.canvas", BOARD);
    let mut metalmind = smoke.launch(&["idea.canvas"]);
    let window = metalmind.window();
    window.wait_title("de la pizarra", |title| title.starts_with("idea.canvas"));
    window.type_keys("xxddihjkl+-!):%d<CR>:w<CR>");
    window.type_keys("<CR>");
    window.wait_title("de la nota", |title| title.starts_with("nota.md"));
    window.type_keys(":q!<CR>");
    assert!(metalmind.wait_exit().success());
    assert_eq!(smoke.read("idea.canvas"), BOARD);
}
