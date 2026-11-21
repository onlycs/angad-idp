import { optimizeComponents, optimizeCss, optimizeImports } from "carbon-preprocess-svelte";
import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

export default defineConfig({
    plugins: [
        tailwindcss(),
        sveltekit({
            adapter: adapter(),
            // fuck you svelte, i won't port over, hashtags are ugly.
            alias: {
                $lib: "./src/lib",
                $routes: "./src/routes",
                $components: "./src/lib/components",
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
