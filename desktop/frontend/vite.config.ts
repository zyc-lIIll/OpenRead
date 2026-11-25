import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const root = fileURLToPath(new URL(".", import.meta.url));
export default defineConfig({ publicDir: resolve(root, "node_modules/.cache/openread-public") });
