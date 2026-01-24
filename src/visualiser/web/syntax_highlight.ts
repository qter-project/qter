import * as TreeSitter from "web-tree-sitter";

export function getRange(el: Node, start: number, end: number): Range {
    let walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    let done = walker.nextNode() == null;

    while (!done) {
        let node = walker.currentNode as Text;
        if (start <= node.length) break;
        start -= node.length;
        end -= node.length;
        done = walker.nextNode() == null;
    }
    let startNode = walker.currentNode as Text;

    while (!done) {
        let node = walker.currentNode as Text;
        if (end <= node.length) break;
        end -= node.length;
        done = walker.nextNode() == null;
    }
    let endNode = walker.currentNode as Text;

    start = Math.min(start, startNode.length);
    end = Math.min(end, endNode.length);

    let range = new Range();
    range.setStart(startNode, start);
    range.setEnd(endNode, end);

    return range;
}

export class SyntaxHighlighter {
    #parser: TreeSitter.Parser;
    #query: TreeSitter.Query;
    #highlightSet: Set<Highlight> = new Set();

    constructor(lang: TreeSitter.Language, query: string) {
        this.#parser = new TreeSitter.Parser();
        this.#query = new TreeSitter.Query(lang, query);
        this.#parser.setLanguage(lang);
    }

    highlight(el: Node) {
        function getOrInsert<K, V>(map: Map<K, V>, key: K, val: () => V): V {
            let v;
            return map.get(key) ?? (map.set(key, v = val()), v);
        }

        let parsed = this.#parser.parse(el.textContent ?? "")!;
        for (let capture of this.#query.captures(parsed.rootNode)) {
            let range = getRange(el, capture.node.startIndex, capture.node.endIndex);

            let highlight = getOrInsert(CSS.highlights, capture.name, () => {
                let newHl = new Highlight();
                this.#highlightSet.add(newHl);
                return newHl;
            });
            highlight.add(range);
        }
    }

    clear() {
        this.#highlightSet.forEach(hl => hl.clear());
    }
}