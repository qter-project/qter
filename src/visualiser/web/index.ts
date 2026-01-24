import * as TreeSitter from "web-tree-sitter";
import { CompileError, CubeState, Interpreter, Program, type Register, type RegisterState } from "./visualiser.js"
import { CubeElement, CubePairElement } from "./cube_view.js";
import { getRange, SyntaxHighlighter } from "./syntax_highlight.js";

function cycleColor(regIdx: number, cycleIdx: number): string {
    return `oklch(${0.76 - cycleIdx * 0.15} 0.12 ${regIdx / 4 * 360 + 240})`;
}

class RegisterElement extends HTMLElement {
    #shadowRoot: ShadowRoot | null = null;
    #valueSpan: HTMLSpanElement | null = null;
    #cycleSpans: HTMLSpanElement[] | null = null;

    #index: number;
    #label: string;
    #order: number;
    #cycleOrders: number[];
    #value: number | null;
    #cycleValues: (number | null)[];

    constructor(index: number, label: string, order: number, cycleOrders: number[]) {
        super();

        this.#index = index;
        this.#label = label;
        this.#order = order;
        this.#cycleOrders = cycleOrders;
        this.#value = null;
        this.#cycleValues = Array.from(cycleOrders, () => null);
    }

    connectedCallback() {
        if (this.#shadowRoot == null) {
            this.#shadowRoot = this.attachShadow({ mode: "closed" });
        }

        let container = document.createElement("div");
        container.id = "container";

        let label = document.createElement("span");
        label.classList.add("label");
        label.textContent = this.#label;
        container.appendChild(label);

        let eq = document.createElement("span");
        eq.textContent = " = ";
        container.appendChild(eq);

        let value = document.createElement("span");
        value.classList.add("value");
        let current = document.createElement("span");
        value.append(current, "/", `${this.#order}`);
        container.appendChild(value);

        let cycleSpans = [];
        for (let [i, cycleOrder] of this.#cycleOrders.entries()) {
            let cycle = document.createElement("span");
            cycle.classList.add("cycle");
            cycle.style.setProperty("--color", cycleColor(this.#index, i));
            let current = document.createElement("span");
            cycle.append(current, "/", `${cycleOrder}`);
            container.append(cycle);

            cycleSpans.push(current);
        }

        let style = document.createElement("style");
        style.textContent = `
            :host {
                display: block;
            }
            
            #container {
                display: flex;
                flex-flow: row nowrap;
                align-items: center;
            }

            .label {
                font-size: 1.5em;
            }
            
            .value {
                font-size: 1.25em;
                margin-right: 1em;
            }

            .cycle {
                margin-left: 0.3em;
                padding: 0.2em;
                background-color: var(--color);
            }
        `;

        this.#shadowRoot.appendChild(style);
        this.#shadowRoot.appendChild(container);
        this.#valueSpan = current;
        this.#cycleSpans = cycleSpans;
        this.#updateValue();
    }

    disconnectedCallback() {
        this.#shadowRoot!.replaceChildren();
    }

    #updateValue() {
        if (this.#valueSpan != null) {
            this.#valueSpan!.textContent = `${this.#value ?? "-"}`;
            for (let [i, value] of this.#cycleValues.entries()) {
                this.#cycleSpans![i]!.textContent = `${value ?? "-"}`;
            }
        }
    }

    clearValue() {
        this.#value = null;
        this.#cycleValues = Array.from(this.#cycleValues, () => null);
    }

    setValue(value: number, cycleValues: number[]) {
        if (cycleValues.length != this.#cycleOrders.length) throw new Error();
        this.#value = value;
        this.#cycleValues = [...cycleValues];
        this.#updateValue();
    }

    static {
        customElements.define("register-view", RegisterElement);
    }
}

class Editor extends EventTarget {
    #text: HTMLPreElement;
    #highlighter: SyntaxHighlighter | null;

    #disabled: boolean = false;

    constructor(text: HTMLPreElement, hl?: SyntaxHighlighter | null) {
        super();
        this.#text = text;
        this.#text.addEventListener("input", this.#onInput.bind(this));
        this.#text.addEventListener("keydown", this.#onKeyDown.bind(this));
        this.#highlighter = hl ?? null;

        this.disabled = this.#disabled;
        this.#highlight();
    }

    get disabled(): boolean {
        return this.#disabled;
    }

    set disabled(value: boolean) {
        this.#disabled = value;
        this.#text.contentEditable = value ? "false" : "plaintext-only";
    }

    declare addEventListener: EventTarget["addEventListener"]
        & ((type: "input", callback: (ev: CustomEvent<string>) => void) => void);

    #onInput(ev: Event) {
        if (this.#disabled) {
            ev.preventDefault();
            return;
        }

        this.#highlight();
        this.dispatchEvent(new CustomEvent<string>("input", { detail: this.#text.textContent }));
    }

    #onKeyDown(ev: KeyboardEvent) {
        // TODO: better editor
        if (ev.key == "Tab") {
            let selection = getSelection()!;
            let range = selection.getRangeAt(0);
            if (this.#text.contains(range.startContainer) && this.#text.contains(range.endContainer)) {
                ev.preventDefault();
                range.deleteContents();
                range.insertNode(new Text("    "));
                range.collapse(false);
            }
        }
    }

    get text(): string {
        return this.#text.textContent;
    }

    set text(value: string) {
        this.#text.textContent = value;
        this.#highlight();
    }

    #highlight() {
        if (this.#highlighter) {
            this.#highlighter.clear();
            this.#highlighter.highlight(this.#text);
        }
    }

    getRange(start: number, end: number): Range {
        return getRange(this.#text, start, end);
    }
}

type Preset = {
    name: string,
    code: string,
    precompiled: Program | null,
};

type SavedCode = {
    description: string,
    name: string,
    code: string,
}

class EditorWithPresets extends EventTarget {
    #editor: Editor;
    #presetSelector: HTMLSelectElement;
    #dialog: HTMLDialogElement;
    #dialogForm: HTMLFormElement;
    #presets: readonly Preset[];

    #customOption: HTMLOptionElement;
    #customText: string = "";
    #savedCodeOptgroup: HTMLOptGroupElement;

    static readonly SAVES_KEY = "saved_code";

    constructor(inner: Editor, presetSelector: HTMLSelectElement, dialog: HTMLDialogElement, presets: readonly Preset[]) {
        function optgroup(label: string): HTMLOptGroupElement {
            let v = document.createElement("optgroup");
            v.label = label;
            return v;
        }

        super();
        this.#editor = inner;
        this.#editor.addEventListener("input", this.#onCodeInput.bind(this));
        this.#presetSelector = presetSelector;
        this.#presetSelector.addEventListener("change", this.#onSelect.bind(this));
        this.#dialog = dialog;
        this.#dialogForm = dialog.querySelector("form")!;
        this.#dialog.addEventListener("close", this.#onDialogClose.bind(this));
        this.#presets = presets;

        let presetsOptgroup = optgroup("Presets");
        for (let [i, preset] of this.#presets.entries()) {
            presetsOptgroup.appendChild(new Option(preset.name, `${i}`));
        }
        this.#presetSelector.add(presetsOptgroup);

        this.#presetSelector.add(this.#customOption = new Option("Custom", "custom"));
        this.#presetSelector.add(this.#savedCodeOptgroup = optgroup("Saved code"));
        this.#customOption.selected = true;

        window.addEventListener("storage", ev => {
            if (ev.storageArea !== localStorage) return;
            if (ev.key !== EditorWithPresets.SAVES_KEY) return;
            this.#savedCode = ev.newValue != null ? JSON.parse(ev.newValue) as SavedCode[] : [];
        });

        let currentVal: string | null = localStorage.getItem(EditorWithPresets.SAVES_KEY);
        if (currentVal == null) {
            this.#savedCode = [];
            this.#persistSavedCode();
        } else {
            this.#savedCode = JSON.parse(currentVal);
        }
    }

    get disabled(): boolean {
        return this.#editor.disabled;
    }

    set disabled(value: boolean) {
        this.#editor.disabled = value;
        this.#presetSelector.disabled = value;
    }

    declare addEventListener: EventTarget["addEventListener"]
        & ((type: "change", callback: (ev: CustomEvent<[string, Program | null]>) => void) => void);

    #dispatchChangeEvent(text: string, precompiled: Program | null) {
        this.dispatchEvent(new CustomEvent<[string, Program | null]>("change", { detail: [text, precompiled] }));
    }

    #onSelect(ev: Event) {
        if (this.disabled) {
            ev.preventDefault();
            return;
        }

        let value = this.#presetSelector.value;

        if (value == "") {
            ev.preventDefault();
        } else if (value == "custom") {
            this.#editor.text = this.#customText;
            this.#dispatchChangeEvent(this.#customText, null);
        } else if (value.startsWith("saved-")) {
            let idx = Number(value.substring(6));
            let saved = this.#savedCode[idx]!;
            this.#editor.text = saved.code;
            this.#dispatchChangeEvent(saved.code, null);
        } else {
            let idx = Number(value);
            let preset = this.#presets[idx]!;
            this.#editor.text = preset.code;
            this.#dispatchChangeEvent(preset.code, preset.precompiled);
        }
    }

    #onCodeInput(ev: CustomEvent<string>) {
        this.#customOption.selected = true;
        this.#customText = ev.detail;
        this.#dispatchChangeEvent(this.#customText, null);
    }

    #onDialogClose(ev: Event) {
        let desc = (this.#dialogForm.elements.namedItem("desc")! as HTMLInputElement).value;
        let name = (this.#dialogForm.elements.namedItem("name")! as HTMLInputElement).value;
        this.#dialogForm.reset();

        if (this.#dialog.returnValue != "save") {
            return;
        }

        let code = this.#editor.text; // TODO: use the code from the time the dialog was opened?

        let opt = this.#pushSavedCode({
            description: desc,
            name,
            code
        });
        this.#persistSavedCode();

        opt.selected = true; // hm, but then it [the thing in the above TODO] would break this...
    }

    #_savedCode: SavedCode[] = [];
    get #savedCode(): SavedCode[] {
        return this.#_savedCode;
    }

    set #savedCode(value: SavedCode[]) {
        this.#_savedCode = value;
        this.#savedCodeOptgroup.replaceChildren();
        for (let [i, preset] of value.entries()) {
            this.#savedCodeOptgroup.appendChild(new Option(`${preset.description} - ${preset.name}`, `saved-${i}`));
        }
    }

    #pushSavedCode(newValue: SavedCode): HTMLOptionElement {
        let opt = new Option(`${newValue.description} - ${newValue.name}`, `saved-${this.#_savedCode.length}`);
        this.#savedCodeOptgroup.appendChild(opt);
        this.#_savedCode.push(newValue);
        return opt;
    }

    #persistSavedCode() {
        window.localStorage.setItem(EditorWithPresets.SAVES_KEY, JSON.stringify(this.#savedCode));
    }

    getRange(start: number, end: number): Range {
        return this.#editor.getRange(start, end);
    }
}

class Output {
    #output: HTMLPreElement;
    #highlighter: SyntaxHighlighter | null;

    constructor(output: HTMLPreElement, hl?: SyntaxHighlighter | null) {
        this.#output = output;
        this.#highlighter = hl ?? null;
    }

    setText(value: string, highlight: boolean = true) {
        this.#output.textContent = value;
        this.#highlight(highlight);
    }

    #highlight(highlight: boolean) {
        if (this.#highlighter) {
            this.#highlighter.clear();
            if (highlight) this.#highlighter.highlight(this.#output);
        }
    }

    getRange(start: number, end: number): Range {
        return getRange(this.#output, start, end);
    }
}

class EditorWithCompilation extends EventTarget {
    #editor: EditorWithPresets;
    #output: Output;

    #errorHighlight: Highlight;

    constructor(inner: EditorWithPresets, output: Output) {
        super();
        this.#editor = inner;
        this.#editor.addEventListener("change", this.#onCodeChange.bind(this));
        this.#output = output;

        CSS.highlights.set("error", this.#errorHighlight = new Highlight());
    }

    get disabled(): boolean {
        return this.#editor.disabled;
    }

    set disabled(value: boolean) {
        this.#editor.disabled = value;
    }

    declare addEventListener: EventTarget["addEventListener"]
        & ((type: "change", callback: (ev: CustomEvent<null | Program>) => void) => void);

    #dispatchChangeEvent(compiled: null | Program) {
        this.dispatchEvent(new CustomEvent<null | Program>("change", { detail: compiled }));
    }

    #onCodeChange(ev: CustomEvent<[string, Program | null]>) {
        let [code, precompiled] = ev.detail;

        this.#errorHighlight.clear();

        let compiled: Program;
        try {
            compiled = precompiled ?? new Program(code);

            this.#output.setText(compiled.q_text());

            this.#dispatchChangeEvent(compiled);
        } catch (e) {
            let msg;
            if (Array.isArray(e) && (e.length == 0 || e[0] instanceof CompileError)) {
                msg = "";
                for (let error of e as CompileError[]) {
                    msg += `error at ${error.start_line()}:${error.start_col()}: ${error.message()}\n`;
                    this.#errorHighlight.add(this.#editor.getRange(error.start(), error.end()));
                }
                msg = msg.trimEnd();
            } else {
                console.error(e);
                msg = `Unexpected error: ${e}`;
            }
            this.#output.setText(msg, false);

            this.#dispatchChangeEvent(null);
        }
    }

    getOutputRange(start: number, end: number): Range {
        return this.#output.getRange(start, end);
    }
}

class Infoview {
    #stateCube: CubePairElement;
    #registersCube: CubePairElement;
    #registersContainer: HTMLDivElement;

    #registers: RegisterElement[];
    #compiledState: Register[] | null = null;
    #runtimeState: CubeState | null = null;

    // static FACE_COLOURS = [
    //     "white",
    //     "orange",
    //     "lightgreen",
    //     "red",
    //     "dodgerblue",
    //     "yellow",
    // ] as const;
    static FACE_COLOURS = [
        "rgb(255 255 255)",
        "rgb(255 128 0)",
        "rgb(0 255 0)",
        "rgb(255 0 0)",
        "rgb(0 0 255)",
        "rgb(255 255 0)",
    ] as const;

    static BLANK_COLOUR = "gray";

    static BLANK_CUBE: [string, string][] & { length: 54 } =
        Array.from(new Array(54), () => [Infoview.BLANK_COLOUR, ""]) as [string, string][] & { length: 54 };
    // static SOLVED_CUBE: [string, string][] & { length: 54 } =
    //     Infoview.#faceletDataFromCubeState(Array.from(new Array(48), (_, i) => i));

    constructor(
        stateCube: CubePairElement,
        registersCube: CubePairElement,
        registers: HTMLDivElement
    ) {
        this.#stateCube = stateCube;
        this.#registersCube = registersCube;
        this.#registersContainer = registers;

        this.#registers = [];

        this.program = null;
    }

    static #faceletDataFromCubeState(data: number[]): [string, string][] & { length: 54 } {
        if (data.length != 48) throw new Error();

        let res: [string, string][] = Array.from(new Array(54), (_, i) => {
            let face;
            if (i % 9 == 4) {
                face = Math.floor(i / 9);
            } else {
                i = i % 9 + Math.floor(i / 9) * 8 - (i % 9 > 4 ? 1 : 0);
                face = Math.floor(data[i]! / 8);
            }
            let color = Infoview.FACE_COLOURS[face]!;
            return [color, ""];
        });
        return res as typeof res & { length: 54 };
    }

    static #faceletDataForRegisters(registers: Register[]): [string, string][] & { length: 54 } {
        let out: [string, string][] = Array.from(this.BLANK_CUBE, ([v, u]) => [v, u]);
        for (let i = 0; i < 6; i++) {
            out[i * 9 + 4]![0] = Infoview.FACE_COLOURS[i]!;
        }

        for (let [i, register] of registers.entries()) {
            for (let [j, cycle] of register.cycles.entries()) {
                for (let [idx, facelet] of cycle.facelets.entries()) {
                    facelet = facelet % 8 + (facelet % 8 >= 4 ? 1 : 0) + Math.floor(facelet / 8) * 9;
                    out[facelet]![0] = cycleColor(i, j);
                    out[facelet]![1] = `${idx + 1}`;
                }
            }
        }

        return out as typeof out & { length: 54 };
    }

    set program(value: Program | null) {
        if (value == null) {
            this.#compiledState = null;
            this.#registers = [];
            this.#stateCube.setFaceletData(Infoview.BLANK_CUBE);
            this.#registersCube.setFaceletData(Infoview.BLANK_CUBE);
        } else {
            this.#compiledState = value.registers();
            this.#registers = this.#compiledState.map((v, i) =>
                new RegisterElement(i, v.label, Number(v.order), v.cycles.map(cycle => Number(cycle.order))));
            this.#stateCube.setFaceletData(Infoview.BLANK_CUBE);
            this.#registersCube.setFaceletData(Infoview.#faceletDataForRegisters(this.#compiledState));
        }
        this.#registersContainer.replaceChildren(...this.#registers);
        this.#runtimeState = null;
    }

    set state(value: CubeState | null) {
        if (this.#compiledState == null || value != null && this.#compiledState.length != value.registers.length) {
            throw new Error("set infoview state without proper compiled program");
        }

        this.#runtimeState = value;
        if (value == null) {
            for (let register of this.#registers) register.clearValue();
            this.#stateCube.setFaceletData(Infoview.BLANK_CUBE);
        } else {
            for (let [i, register] of this.#registers.entries()) {
                let regVal = value.registers[i]!;
                register.setValue(Number(regVal.value), regVal.cycle_values.map(Number));
            }
            this.#stateCube.setFaceletData(Infoview.#faceletDataFromCubeState(value.cube.facelets));
        }
    }
}

let presets: Preset[] = [];
export async function compilePresets() {
    const { default: presetPaths } = await import("./presets/presets.json", { with: { type: "json" } });

    let res = [];
    for (let { name, file } of presetPaths) {
        let code = await fetch(`/presets/${file}`).then(v => v.text());
        res.push({
            name,
            code,
            precompiled: new Program(code)
        });
    }
    presets = res;
}

let qSyntax: SyntaxHighlighter | null = null;
let qatSyntax: SyntaxHighlighter | null = null;
export async function loadHighlighting() {
    qSyntax = new SyntaxHighlighter(
        await TreeSitter.Language.load('./tree-sitter-qter_q.wasm'),
        await fetch("./highlights.scm").then(v => v.text()),
    );
}

export function main() {
    const editorCodeContainer = document.getElementById("editor-code-container") as HTMLPreElement;
    const presetSelector = document.getElementById("preset-selector") as HTMLSelectElement;
    const saveCodeDialog = document.getElementById("save-code-dialog") as HTMLDialogElement;
    const compiledCodeOutput = document.getElementById("compiled-code-output") as HTMLPreElement;
    const registersCube = document.getElementById("registers-cube") as CubePairElement;
    const stateCube = document.getElementById("state-cube") as CubePairElement;
    const registersContainer = document.getElementById("registers") as HTMLDivElement;

    let editor = (window as any).editor = new EditorWithCompilation(
        new EditorWithPresets(
            new Editor(editorCodeContainer, qatSyntax),
            presetSelector,
            saveCodeDialog,
            presets,
        ),
        new Output(compiledCodeOutput, qSyntax)
    );

    let infoview = (window as any).infoview = new Infoview(
        stateCube,
        registersCube,
        registersContainer,
    );

    let abort: AbortController | null = null;
    editor.addEventListener("change", ev => {
        let program = ev.detail;

        if (abort != null) {
            abort.abort();
            abort = null;
        }

        infoview.program = program;
        abort = new AbortController();
        let signal = abort.signal;

        (async () => {
            if (program != null) {
                let interpreter = await Interpreter.init(program, null as any, null as any, cube => {
                    infoview.state = cube;
                });

                if (signal.aborted) return;

                while (true) {
                    let res = await interpreter.step();
                    if (signal.aborted) return;
                    if (res.kind == "Running") { }
                    else if (res.kind == "NeedsInput") {
                        // let input = Math.floor(Number(res.max_input) / 2);
                        let input = Math.floor(Math.random() * 90);
                        console.log("max: ", res.max_input, " @ ", interpreter.program_counter());
                        await interpreter.give_input(BigInt(input));
                        if (signal.aborted) return;
                    }
                    console.log(interpreter.program_counter(), interpreter.messages());
                    let { start, end } = program.instr_span(interpreter.program_counter());
                    if (CSS.highlights.get("current-instr") == null) CSS.highlights.set("current-instr", new Highlight());
                    CSS.highlights.get("current-instr")!.clear();
                    if (res.kind == "Halted") break;
                    CSS.highlights.get("current-instr")!.add(editor.getOutputRange(start, end));
                    await new Promise(resolve => setTimeout(resolve, 500));
                    if (signal.aborted) return;
                }
            }
        })();
    })
}