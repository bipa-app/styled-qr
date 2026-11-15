/** What `render` takes: every field is optional. */
export type QrOptions = {
    /** Error correction; H by default, which leaves room for a logo. */
    ec?: "L" | "M" | "Q" | "H";
    /** Pixels per module; 11 by default. */
    modulePx?: number;
    /** Empty modules around the code; 2 by default. */
    quietZone?: number;
    shape?: "dots" | "rounded" | "connected" | "square";
    finder?: "rounded" | "square";
    /** `#rrggbb`. `render` refuses a pair under `minContrast()`. */
    colors?: { dark: string; light: string; finder?: string };
    /** Applies when `render` receives a logo. */
    logo?: {
        /** Share of the code's width, in (0, 1]. */
        width?: number;
        /** Empty modules around the logo; 2 by default. */
        padding?: number;
        plate?: { color: string; margin: number; roundness: number };
        /** Link the SVG's logo from this URL instead of embedding it, for a page that serves the
         *  logo once. Only an SVG inline in HTML loads it; the PNG always embeds the logo. */
        href?: string;
    };
    /** Also return the PNG. */
    png?: boolean;
    /** Center the PNG's code on a larger canvas, e.g. a 1200 × 630 link preview; the SVG stays
     *  the code alone. `color` is `#rrggbb`, the code's light color by default. A canvas smaller
     *  than the code is refused as `options`. */
    canvas?: { width: number; height: number; color?: string };
};

export type QrRefusal = {
    code: "contrast" | "logoUnreadable" | "logoImage" | "color" | "data" | "tooLarge" | "options";
    message: string;
};
