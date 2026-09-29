// @vitest-environment node
import { expect, it } from "vitest";
import { artworkFor } from "../electron/artwork-policy";

it("offers only the manufacturer's wired Apex Pro TKL Gen 3 photograph for 1038:1642", () => {
  const artwork = artworkFor(0x1038, 0x1642);
  expect(artwork?.model).toBe("Apex Pro TKL Gen 3");
  expect(artwork?.sourceURL).toMatch(/^https:\/\/steelseries\.com\/gaming-keyboards\/apex-pro-gen-3\?/);
  expect(artwork?.sourceURL).toContain("keyboardSize=tkl&connectivityType=wired&color=black");
  expect(artwork?.imageURL).toMatch(/^https:\/\/images\.ctfassets\.net\/hmm5mo4qf4mf\//);
  expect(artwork?.imageURL).toContain("apex_pro_tkl_gen_3_black_img_buy_01");
  expect(artwork?.variant).toMatch(/black/i);
  expect(artwork?.variant).toMatch(/USB.*(?:color|colour)/i);
  expect(artwork?.attribution).toContain("© SteelSeries");
});

it("refuses a wrong generation, a wireless product, an unknown device, and another vendor", () => {
  for (const [vendor, product] of [
    [0x1038, 0x1628], // older experimental Apex: not Gen 3
    [0x1038, 0x1630], // wireless Apex TKL (2023)
    [0x1038, 0x1632], // wireless Apex TKL (2023)
    [0x1038, 0x1643], // adjacent ID is not evidence
    [0x1038, 0xffff],
    [0x1234, 0x1642],
  ]) expect(artworkFor(vendor, product)).toBeNull();
  expect(artworkFor(0x1038, 0x227e)?.model).toBe("Arctis Nova 7 Gen 2");
});
