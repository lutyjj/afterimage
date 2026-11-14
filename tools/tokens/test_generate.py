"""The generator's reading of the stylesheet's rules, from computed values the
release gives on TRMNL's own displays (Chromium, framework 3.4.6)."""

import unittest

from generate import pattern

BLACK, WHITE = "#000000", "#FFFFFF"
# The rule under a table's headings: black and white, two device pixels.
HEAD = (
    "linear-gradient(to right, rgb(0, 0, 0) 0%, rgb(0, 0, 0) 50%, rgb(255, 255, 255) 50%, "
    "rgb(255, 255, 255) 100%), linear-gradient(to right, rgb(0, 0, 0) 0%, rgb(0, 0, 0) 50%, "
    "rgb(255, 255, 255) 50%, rgb(255, 255, 255) 100%)"
)
# The rule between body rows: one black pixel, four white, one transparent.
BODY = (
    "linear-gradient(to right, rgb(0, 0, 0) 0%, rgb(0, 0, 0) 8.33333%, rgb(255, 255, 255) 8.33333%, "
    "rgb(255, 255, 255) 41.6667%, rgba(0, 0, 0, 0) 41.6667%, rgba(0, 0, 0, 0) 50%, rgb(0, 0, 0) 50%, "
    "rgb(0, 0, 0) 58.3333%, rgb(255, 255, 255) 58.3333%, rgb(255, 255, 255) 91.6667%, "
    "rgba(0, 0, 0, 0) 91.6667%, rgba(0, 0, 0, 0) 100%)"
)


class Pattern(unittest.TestCase):
    def test_a_rule_at_one_css_pixel_a_device_pixel(self):
        self.assertEqual(
            pattern(HEAD, "2px 1px, 2px 1px", 1.0),
            {"pixels": [BLACK, WHITE], "period": 2.0, "thickness": 1.0},
        )
        row = [BLACK, WHITE, WHITE, WHITE, WHITE, None]
        self.assertEqual(pattern(BODY, "12px 1px, 12px 1px", 1.0)["pixels"], row * 2)

    def test_a_scaled_display_keeps_its_device_pixels(self):
        # TRMNL X draws 1.8 device pixels to a CSS pixel; the stylesheet sizes
        # the rule in device pixels, so its tile is 1.11 CSS pixels wide.
        self.assertEqual(
            pattern(HEAD, "1.11111px 0.555556px, 1.11111px 0.555556px", 1.8),
            {"pixels": [BLACK, WHITE], "period": 1.1111, "thickness": 0.5556},
        )
        row = [BLACK, WHITE, WHITE, WHITE, WHITE, None]
        self.assertEqual(pattern(BODY, "6.66667px 0.555556px", 1.8)["pixels"], row * 2)

    def test_one_colour_is_a_plain_rule(self):
        rule = pattern("linear-gradient(rgb(204, 204, 204), rgb(204, 204, 204))", "100% 1px", 1.0)
        self.assertEqual((rule["pixels"], rule["thickness"]), (["#CCCCCC"], 1.0))

    def test_what_is_not_a_rule_across_is_none(self):
        self.assertIsNone(pattern("none", "auto", 1.0))
        down = HEAD.replace("to right", "to bottom")
        self.assertIsNone(pattern(down, "2px 1px", 1.0))
        # Without a direction, CSS runs it to the bottom.
        unstated = HEAD.replace("to right, ", "")
        self.assertIsNone(pattern(unstated, "2px 1px", 1.0))


if __name__ == "__main__":
    unittest.main()
