export class RotationController extends EventTarget {
    #pitch: number;
    #yaw: number;

    constructor(pitch: number = 30, yaw: number = -40) {
        super();
        this.#pitch = pitch;
        this.#yaw = yaw;
    }

    declare addEventListener: EventTarget["addEventListener"]
        & ((type: "update", callback: (ev: Event) => void) => void);

    setRotation(yaw: number, pitch: number) {
        this.#yaw = yaw;
        this.#pitch = Math.min(90, Math.max(-90, pitch));
        this.dispatchEvent(new Event("update"));
    }

    addRotation(yaw: number, pitch: number) {
        this.setRotation(this.#yaw + yaw, this.#pitch + pitch);
    }

    get pitch(): number { return this.#pitch; }
    get yaw(): number { return this.#yaw; }

    registerRotateOnDrag(el: HTMLElement) {
        el.addEventListener("wheel", (event: WheelEvent) => {
            if (event.deltaMode != WheelEvent.DOM_DELTA_PIXEL) return;
            let x = event.deltaX;
            let y = event.deltaY;
            event.preventDefault();

            this.addRotation(x / 5, y / 5);
        }, { passive: false });

        let abort: AbortController | null = null;
        const mouseMoveListener = (ev: MouseEvent) => {
            this.addRotation(ev.movementX / 3, ev.movementY / 3);
        };
        document.addEventListener("pointerlockchange", () => {
            if (document.pointerLockElement === el) {
                if (abort != null) return;
                abort = new AbortController();
                el.addEventListener("mousemove", mouseMoveListener, { signal: abort.signal });
                el.addEventListener("mouseup", () => {
                    document.exitPointerLock();
                }, { signal: abort.signal });
            } else {
                abort?.abort();
                abort = null;
            }
        });
        el.addEventListener("mousedown", () => {
            el.requestPointerLock();
        })
    }
}

export class CubeElement extends HTMLElement {
    #shadowRoot: ShadowRoot | null = null;
    #viewport: HTMLElement | null = null;
    #cube: HTMLElement | null = null;
    #facelets: [HTMLDivElement, HTMLSpanElement][] & { length: 54 } | null = null;

    #rotation: RotationController | null = null;
    #invert: boolean = false;

    static #resizeObserver = new ResizeObserver((entries) => {
        for (const entry of entries) {
            let target = entry.target as CubeElement;
            target.#onresize(entry.contentBoxSize[0]!);
        }
    })

    constructor() {
        super();
    }

    private connectedCallback() {
        if (this.#shadowRoot == null) {
            this.#shadowRoot = this.attachShadow({ mode: "closed" });
        }

        let viewport = document.createElement("div");
        viewport.id = "viewport";

        let cube = document.createElement("div");
        cube.id = "cube";
        viewport.appendChild(cube);

        // let colours = ["white", "orange", "lime", "red", "blue", "yellow"];
        let transforms = ["rotateX(90deg)", "rotateY(-90deg)", " ", "rotateY(90deg)", "rotateY(180deg)", "rotateX(-90deg)"];
        let facelets: [HTMLDivElement, HTMLSpanElement][] = [];
        for (let i = 0; i < 6; i++) {
            let face = document.createElement("div");
            face.classList.add("face");
            face.style.setProperty("--face-transform", transforms[i]!);
            // face.style.setProperty("--face-color", colours[i]);
            cube.appendChild(face);

            for (let j = 0; j < 9; j++) {
                let facelet = document.createElement("div");
                facelet.classList.add("facelet");
                facelet.style.setProperty("--color", "white");
                face.appendChild(facelet);

                let text = document.createElement("span");
                text.classList.add("text");
                text.textContent = "";
                facelet.appendChild(text);

                facelets.push([facelet, text]);
            }
        }

        let style = document.createElement("style");
        style.textContent = `
            :host {
                display: block;
                aspect-ratio: 1;
                contain: strict;
            }
            
            #viewport {
                perspective: calc(var(--cube-size) * 4);
                width: 100%;
                height: 100%;
                display: flex;
                align-items: center;
                justify-content: center;
            }
            
            #cube {
                transform-style: preserve-3d;
                width: var(--cube-size);
                height: var(--cube-size);
            }
            
            .face {
                position: absolute;
                box-sizing: border-box;
                width: var(--cube-size);
                aspect-ratio: 1;
                transform: var(--face-transform) translateZ(calc(var(--cube-size) / 2));
                /* transform-style: flat; */
                background-color: black;
                display: grid;
                grid-template-rows: repeat(3, 1fr);
                grid-template-columns: repeat(3, 1fr);
                gap: 2%;
                padding: 1%;
            }
            
            #cube.inverted .face {
                transform: var(--face-transform) translateZ(calc(var(--cube-size) / 2)) scaleZ(-1);
                backface-visibility: hidden;
            }
            
            .facelet {
                /* border: 2px solid color(from var(--color) srgb r g b / 90%); */
                border-radius: 10%;
                /* background-color: hsl(from var(--color) h calc(0.5 * s) l / 90%); */
                background-color: hsl(from var(--color) h calc(0.7 * s) l);
                aspect-ratio: 1;
                font-size: calc(var(--cube-size) * 1/8);
                color: if(
                    style(--color: white): black;
                    style(--color: yellow): black;
                    else: white;
                );

                display: flex;
                align-items: center;
                justify-content: center;
            }
            
            #cube.inverted .facelet {
                transform: scaleX(-1);
            }
        `;

        this.#shadowRoot.appendChild(style);
        this.#shadowRoot.appendChild(viewport);
        this.#viewport = viewport;
        this.#cube = cube;
        this.#facelets = facelets as (typeof facelets & { length: 54 });
        CubeElement.#resizeObserver.observe(this);
        this.#updateRotation();
        this.#updateInverted();
    }

    private disconnectedCallback() {
        this.#shadowRoot!.replaceChildren();
        CubeElement.#resizeObserver.unobserve(this);
    }

    #onresize(size: ResizeObserverSize) {
        this.#viewport!.style.setProperty("--cube-size", `${Math.min(size.inlineSize, size.blockSize) / 1.8}px`)
    }

    #updateRotationEventListener = this.#updateRotation.bind(this);
    #updateRotation() {
        if (this.#cube != null) {
            let pitch = this.#rotation?.pitch ?? 0;
            let yaw = this.#rotation?.yaw ?? 0;
            this.#cube.style.transform = `rotateX(${-pitch}deg) rotateY(${yaw}deg)`;
        }
    }

    setRotation(rotation: RotationController | null) {
        this.#rotation?.removeEventListener("update", this.#updateRotationEventListener);
        this.#rotation = rotation;
        rotation?.addEventListener("update", this.#updateRotationEventListener);
        this.#updateRotation();
    }

    #updateInverted() {
        this.#cube?.classList.toggle("inverted", this.#invert);
    }

    get inverted(): boolean { return this.#invert; }
    set inverted(inverted: boolean) {
        this.#invert = inverted;
        this.#updateInverted();
    }

    setFaceletData(data: readonly (readonly [string, string])[] & { length: 54 }) {
        if (!this.#facelets) return;
        for (let i = 0; i < 54; i++) {
            this.#facelets[i]![0].style.setProperty("--color", data[i]![0]);
            this.#facelets[i]![1].textContent = data[i]![1];
        }
    }

    static {
        customElements.define("cube-view", CubeElement);
    }
}

export class CubePairElement extends HTMLElement {
    #shadowRoot: ShadowRoot | null = null;
    #view1: CubeElement | null = null;
    #view2: CubeElement | null = null;

    #yaw: number = 30;
    #pitch: number = 40;

    constructor() {
        super();
    }

    private connectedCallback() {
        if (this.#shadowRoot == null) {
            this.#shadowRoot = this.attachShadow({ mode: "closed" });
        }

        let container = document.createElement("div");
        container.id = "container";

        container.appendChild(this.#view1 = new CubeElement());
        container.appendChild(this.#view2 = new CubeElement());
        this.#view2.inverted = true;

        let style = document.createElement("style");
        style.textContent = `
            :host {
                display: block;
            }
            
            #container {
                display: flex;
                flex-flow: row nowrap;
                justify-content: space-around;
            }
            
            #container > * {
                flex: 1;
            }
        `;

        this.#shadowRoot.appendChild(style);
        this.#shadowRoot.appendChild(container);
    }

    private disconnectedCallback() {
        this.#shadowRoot!.replaceChildren();
    }

    setRotation(rotation: RotationController | null) {
        this.#view1!.setRotation(rotation);
        this.#view2!.setRotation(rotation);
    }

    setFaceletData(data: readonly (readonly [string, string])[] & { length: 54 }) {
        this.#view1!.setFaceletData(data);
        this.#view2!.setFaceletData(data);
    }

    static {
        customElements.define("cube-pair-view", CubePairElement);
    }
}