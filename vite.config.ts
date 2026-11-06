import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { optimizeComponents, optimizeCss, optimizeImports } from "carbon-preprocess-svelte";
import { defineConfig } from "vite";

export default defineConfig({
    plugins: [
        tailwindcss(),
        sveltekit({
            adapter: adapter(),
            alias: {
                $lib: "./src/lib",
            },
            compilerOptions: {
                runes: ({ filename }) =>
                    filename.split("/").includes("node_modules") ? undefined : true,
            },
            preprocess: [optimizeImports()],
        }),
        optimizeComponents(),
        optimizeCss({ propAware: true }),
    ],
    server: {
        host: true,
        port: 5173,
        allowedHosts: ["idp.dev.angad.page"],
        ws: {
            protocol: "wss",
            host: "idp.dev.angad.page",
            clientPort: 443,
        },
    },
});
