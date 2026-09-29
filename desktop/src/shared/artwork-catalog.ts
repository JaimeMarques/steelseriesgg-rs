// Manufacturer artwork is not included in the code license or the app bundle.
export const nova7Gen2Artwork = {
  model: "Arctis Nova 7 Gen 2",
  variant: "Black; USB identification does not determine casing color",
  imageURL:
    "https://images.ctfassets.net/hmm5mo4qf4mf/2yt5bxGCEbi0Wym2AHqWAQ/c5603bd4ed02257fc86036e15271da62/arctis_nova_7_wl_gen_2_black_pdp_img_buy_01.png",
  sourceURL: "https://steelseries.com/gaming-headsets/arctis-nova-7-gen-2",
  attribution: "Product photograph © SteelSeries. SSGG is an independent project, not affiliated with SteelSeries.",
};
// SteelSeries' wired/TKL/black variant page serves this exact Gen 3 product
// photograph. The image remains © SteelSeries: download only on request and
// cache privately; neither the code license nor distribution grants reuse rights.
// Source identity: https://gitlab.com/CalcProgrammer1/OpenRGB/-/work_items/5249
// documents wired Apex Pro TKL Gen 3 as USB 1038:1642 (not 1038:1628).
export const apexProTklGen3Artwork = {
  model: "Apex Pro TKL Gen 3",
  variant: "Wired TKL, black photograph; USB does not identify color or keyboard layout",
  imageURL:
    "https://images.ctfassets.net/hmm5mo4qf4mf/4JowI26eF16rzKZ2fMJpI7/30f8b3e7ee6f957b5a927f526ada0185/apex_pro_tkl_gen_3_black_img_buy_01.png__1920x1080_crop-fit_optimize_subsampling-2-3759.png",
  sourceURL:
    "https://steelseries.com/gaming-keyboards/apex-pro-gen-3?keyboardLanguage=english&keyboardSize=tkl&connectivityType=wired&color=black",
  attribution: "Product photograph © SteelSeries. SSGG is an independent project, not affiliated with SteelSeries.",
};
export function artworkFor(vendorId: number, productId: number) {
  if (vendorId !== 0x1038) return null;
  if (productId === 0x227e) return nova7Gen2Artwork;
  if (productId === 0x1642) return apexProTklGen3Artwork;
  return null;
}
