import adapter from "@sveltejs/adapter-static";
import { type Config } from "@sveltejs/kit";

export default {
    kit: {
        alias: {
            $static: "static",
            $wasm: "static/wasm",
        },
        adapter: adapter(),
    },
    compilerOptions: {
        runes: ({ filename }) => (filename.split("/").includes("node_modules") ? undefined : true),
    },
} satisfies Config;
