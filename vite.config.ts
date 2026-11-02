import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

export default defineConfig({
    plugins: [
        tailwindcss(),
        sveltekit({
            adapter: adapter(),
            alias: {
                $lib: "./src/lib",
                $wasm: "./static/wasm",
            },
            compilerOptions: {
                runes: ({ filename }) =>
                    filename.split("/").includes("node_modules") ? undefined : true,
            },
        }),
    ],
    server: {
        host: true,
        port: 5173,
        allowedHosts: ["idp.dev.angad.page"],
        hmr: { protocol: "wss", host: "idp.dev.angad.page", clientPort: 443 },
    },
});
