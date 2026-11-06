// The render's entry: the screen, its profile's tokens, the fonts' metrics and its
// images come in as `sys.inputs`.
#import "afterimage.typ": screen
#let inputs = sys.inputs
#screen(json(bytes(inputs.screen)), (..json(bytes(inputs.tokens)), "@metrics": inputs.metrics), inputs.images, inputs.grays, inputs.width, inputs.height)
