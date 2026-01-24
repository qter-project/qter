async function* init() {
    yield "Initialising Tree-sitter...";
    const TreeSitter = await import("web-tree-sitter");
    await TreeSitter.Parser.init();

    yield "Initialising Wasm...";
    const { default: initWasm } = await import("./visualiser.js");
    await initWasm();

    yield "Loading code...";
    let index = await import("./index.js");

    yield "Compiling presets...";
    await index.compilePresets();

    yield "Loading syntax highlighting support...";
    await index.loadHighlighting();

    yield "Starting!";
    index.main();
}

let dialog = document.getElementById("loading-dialog") as HTMLDialogElement;
let messageBox = document.getElementById("loading-message") as HTMLParagraphElement;

let steps = init();
for await (let msg of steps) {
    messageBox.textContent = msg;
}

dialog.close();
document.querySelector("main")!.inert = false;

dialog.remove();