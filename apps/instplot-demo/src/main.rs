use std::path::Path;

use instplot_demo::QuickPlotApp;

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 6 {
        eprintln!("usage: instplot-demo INPUT X_COLUMN Y_COLUMN X_LABEL Y_LABEL OUTPUT.svg");
        std::process::exit(2);
    }
    let mut app = QuickPlotApp::default();
    match app.import_select_label_export(
        Path::new(&arguments[0]),
        &arguments[1],
        &arguments[2],
        &arguments[3],
        &arguments[4],
        Path::new(&arguments[5]),
    ) {
        Ok(bytes) => println!("{} exported {bytes} bytes", app.branding.product_name),
        Err(error) => {
            eprintln!("{}: {error}", app.branding.product_name);
            std::process::exit(1);
        }
    }
}
