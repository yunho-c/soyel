import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("Missing #app mount point");
}

app.innerHTML = `
  <main class="shell">
    <section class="panel">
      <p class="eyebrow">Tauri + Bun + Cargo</p>
      <h1>soyel</h1>
      <form id="greet-form" class="form">
        <label for="name-input">Name</label>
        <div class="row">
          <input id="name-input" name="name" autocomplete="name" value="Tauri" />
          <button type="submit">Greet</button>
        </div>
      </form>
      <p id="greet-output" class="output" aria-live="polite"></p>
    </section>
  </main>
`;

const form = document.querySelector<HTMLFormElement>("#greet-form");
const input = document.querySelector<HTMLInputElement>("#name-input");
const output = document.querySelector<HTMLParagraphElement>("#greet-output");

form?.addEventListener("submit", async (event) => {
  event.preventDefault();

  if (!input || !output) {
    return;
  }

  output.textContent = await invoke<string>("greet", { name: input.value });
});

