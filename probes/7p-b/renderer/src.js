import { EditorView, basicSetup } from "codemirror";
import { javascript } from "@codemirror/lang-javascript";
import katex from "katex";
import "katex/dist/katex.min.css";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { PDFDocument, StandardFonts } from "pdf-lib";
import * as pdfjs from "pdfjs-dist/build/pdf.mjs";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import "./style.css";

// Actual CodeMirror/KaTeX/pdf.js/xterm.js. No fake plaintext renderer substitutes.
async function boot() {
  const edit = new EditorView({
    doc: "const synthetic = (x) => x + 1;\nconsole.log(synthetic(3));",
    extensions: [basicSetup, javascript()],
    parent: document.getElementById("editor")
  });
  if (!edit.dom.querySelector(".cm-content")) throw Error("CodeMirror content absent");

  const formula = "\\frac{a^2+b^2}{\\sqrt{c}}";
  katex.render(formula, document.getElementById("math"), { throwOnError: true });
  if (!document.querySelector(".katex")) throw Error("KaTeX render absent");
  document.getElementById("copy-math").addEventListener("click", async () => {
    await navigator.clipboard.writeText(formula);
  });

  const terminal = new Terminal({ cols: 80, rows: 10, convertEol: true });
  terminal.open(document.getElementById("terminal"));
  terminal.write("\x1b[32mANSI colored fixture\x1b[0m\r\ninput stays synthetic");
  terminal.onData((value) => terminal.write(value));
  document.getElementById("resize-terminal").addEventListener("click", () => terminal.resize(90, 12));
  if (!document.querySelector(".xterm-screen")) throw Error("xterm DOM absent");

  pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;
  const built = await PDFDocument.create();
  const page = built.addPage([420, 260]);
  const font = await built.embedFont(StandardFonts.Helvetica);
  page.drawText("CoSoS PDF searchable synthetic fixture", { x: 20, y: 190, size: 13, font });
  const bytes = new Uint8Array(await built.save());
  const pdf = await pdfjs.getDocument({ data: bytes }).promise;
  const loaded = await pdf.getPage(1);
  const viewport = loaded.getViewport({ scale: 1 });
  const canvas = document.getElementById("pdf-canvas");
  const context = canvas.getContext("2d");
  canvas.width = Math.ceil(viewport.width);
  canvas.height = Math.ceil(viewport.height);
  await loaded.render({ canvasContext: context, viewport }).promise;

  const layer = document.getElementById("pdf-text-layer");
  layer.style.width = viewport.width + "px";
  layer.style.height = viewport.height + "px";
  if (typeof pdfjs.TextLayer !== "function") throw Error("PDF text layer unavailable");
  const textLayer = new pdfjs.TextLayer({
    textContentSource: loaded.streamTextContent(), container: layer, viewport
  });
  await textLayer.render();
  const content = await loaded.getTextContent();
  const searchable = content.items.map((t) => t.str || "").join(" ");
  if (!searchable.includes("searchable synthetic")) throw Error("PDF text extraction mismatch");
  if (!layer.querySelector("span")) throw Error("PDF selectable text layer missing");
  document.getElementById("search-pdf").addEventListener("click", () => {
    document.getElementById("result").textContent =
      searchable.includes("CoSoS PDF searchable") ? "PDF search: matched" : "PDF search: missed";
  });

  document.documentElement.dataset.componentResults = "ready";
  document.documentElement.dataset.rendererReady = "ready";
  document.getElementById("result").textContent = "All four renderer engines initialized.";
}

boot().catch((error) => {
  document.documentElement.dataset.componentResults = "failed";
  document.documentElement.dataset.rendererReady = "failed";
  document.getElementById("result").textContent = "FAIL: " + String(error);
});
