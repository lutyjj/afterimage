$version: "2"

namespace afterimage.management

use smithy.rust#serde

@length(min: 1, max: 64)
string ScreenName

/// A MiniJinja template (Jinja syntax), rendered when the screen is drawn.
///
/// Text without `{{` is itself. A template that is exactly one `{{ expression }}`
/// keeps the expression's value, so `{{ forecast | map(attribute='temperature') }}`
/// is a list; any other template is text. Templates read every data source by its
/// name, `now()` (the time of the render, for filters such as `datetimeformat`)
/// and `device` (`name`, `width`, `height`). Data source requests also read
/// `secrets`. Besides the filters of MiniJinja and minijinja-contrib,
/// `multiply(amount, default)` scales a number as in Home Assistant, so
/// `map('multiply', 0.01)` turns amounts in minor units into a series.
@length(max: 4096)
string Template

/// A name a template reads: a lowercase letter, then lowercase letters, digits or
/// underscores. A data source cannot take a name templates already read: `now`,
/// `device`, `secrets`, `item` or `hidden`.
@pattern("^[a-z][a-z0-9_]{0,31}$")
string Identifier

/// What a screen shows and where its data comes from. Members are the names of
/// the shapes below, so the document reads the same as JSON, as YAML and in the
/// API.
@serde(serialize: true, deserialize: true)
structure ScreenDocument {
    /// HTTP requests whose JSON answers the templates read by name.
    sources: Sources

    /// The bar along the bottom of the screen.
    titleBar: TitleBar

    @required
    content: Component
}

@length(max: 8)
map Sources {
    key: Identifier
    value: Source
}

/// An HTTP request that answers JSON. The server sends it each time it draws the
/// screen and gives up after 10 seconds.
structure Source {
    @required
    url: Template

    method: HttpMethod = "GET"

    headers: Headers

    /// A JSON request body, as a template.
    body: Template
}

enum HttpMethod {
    GET
    POST
}

@length(max: 16)
map Headers {
    key: HeaderName
    value: Template
}

@pattern("^[A-Za-z0-9-]{1,64}$")
string HeaderName

structure TitleBar {
    @required
    title: Template

    /// Shown after the title, such as the place a forecast is for.
    instance: Template

    /// An image URL for the icon before the title.
    icon: Template
}

/// One element of a screen. Text sizes and styles follow TRMNL's design system.
union Component {
    /// Children top to bottom.
    column: Stack

    /// Children left to right.
    row: Stack

    /// Children in equal columns.
    grid: Grid

    /// A block of related text with a bar along its left edge.
    item: Item

    title: Title

    label: Label

    /// A figure or reading.
    value: Value

    description: Description

    image: Image

    /// A line across a column or down a row.
    divider: Divider

    lineChart: Chart

    barChart: Chart

    /// Rows of text under a row of headings, one row for each element of a list.
    table: Table

    /// How far something has come, as a bar that fills.
    progressBar: ProgressBar

    /// Steps done of a number of steps, as a row of dots.
    progressDots: ProgressDots

    /// Paragraphs of text, written in Markdown.
    richText: RichText

    /// Its content once for each element of a list.
    each: Each

    /// One of two components, as a template decides.
    choose: Choose
}

@length(min: 1, max: 32)
list Components {
    member: Component
}

/// How a container's surface is drawn.
@mixin
structure Surface {
    /// Light content on a dark surface, to set one element apart from its
    /// neighbours, such as the cell of today or the active row: TRMNL's inverse.
    inverse: Boolean = false

    /// Space between the edge and the content.
    padding: Gap = "none"

    /// How round the corners are.
    rounded: Rounding = "none"
}

enum Rounding {
    NONE = "none"
    XSMALL = "xsmall"
    SMALL = "small"
    BASE = "base"
    MEDIUM = "medium"
    LARGE = "large"
    XLARGE = "xlarge"
    XXLARGE = "xxlarge"
    FULL = "full"
}

/// Children size to their content; those with `grow` share the space left along
/// the stack's direction.
structure Stack with [Surface] {
    @required
    children: Components

    gap: Gap = "medium"

    /// Across the stack's direction.
    align: Align = "start"

    /// Along the stack's direction, for the space no child takes.
    justify: Justify = "start"

    grow: Boolean = false
}

structure Grid {
    @required
    columns: GridColumns

    @required
    cells: Cells

    gap: Gap = "medium"

    grow: Boolean = false
}

@range(min: 1, max: 12)
integer GridColumns

@length(min: 1, max: 48)
list Cells {
    member: Cell
}

structure Cell {
    @required
    content: Component

    /// Columns the cell spans.
    span: GridColumns = 1
}

structure Item with [Surface] {
    @required
    children: Components

    /// Text in the bar, such as a position in a list.
    index: Template

    /// How dark the bar is: 1, 2 or 3.
    emphasis: Emphasis = 1

    /// Draws no bar.
    plain: Boolean = false

    grow: Boolean = false
}

@range(min: 1, max: 3)
integer Emphasis

structure Title {
    @required
    text: Template

    size: TextSize = "base"

    /// Lines shown before the text is cut with an ellipsis.
    clamp: Lines
}

structure Label {
    @required
    text: Template

    size: TextSize = "base"

    style: LabelStyle = "plain"

    clamp: Lines
}

structure Description {
    @required
    text: Template

    size: TextSize = "base"

    clamp: Lines
}

structure Value {
    @required
    text: Template

    size: ValueSize = "base"

    /// Shrinks the text to fit the width it has.
    fit: Boolean = false

    /// Digits of equal width, so that columns of numbers line up.
    tabular: Boolean = false
}

@range(min: 1, max: 20)
integer Lines

/// It fills what its parent fixes and keeps its proportions: in a row whose
/// height follows its content, an image that grows takes its share of the width
/// and is as tall as that width makes it.
structure Image {
    /// An `http`, `https` or `data` URL of a PNG, JPEG, GIF, WebP or SVG image.
    @required
    url: Template

    fit: ImageFit = "contain"

    grow: Boolean = true
}

/// A line between its neighbours, across a column or down a row, as long as the
/// stack is broad.
structure Divider {}

/// It fills what its parent fixes; where its parent sizes it to content, it is at
/// most 320 pixels wide and half as tall, at most 200 pixels.
structure Chart {
    @required
    series: ChartSeries

    /// The labels along the horizontal axis, as a list.
    labels: Template

    /// The lowest value on the vertical axis; absent fits the data.
    min: Double

    /// The highest value on the vertical axis; absent fits the data.
    max: Double

    grow: Boolean = true
}

/// One row for each element of `items` up to `limit`, and a last row saying how
/// many were left out. Rows are as tall as TRMNL's table rows of the size, or
/// taller when a cell takes more lines; columns share the width as their text
/// does.
structure Table {
    /// The list, as a template.
    @required
    items: Template

    @required
    columns: TableColumns

    size: TableSize = "base"

    /// Numbers the rows in a bar along their left edge.
    indexed: Boolean = false

    /// Rows drawn at most; absent draws at most 100.
    limit: EachLimit

    /// The last row when some are left out; reads `hidden`, the number left out.
    more: Template = "and {{ hidden }} more"

    grow: Boolean = false
}

@length(min: 1, max: 12)
list TableColumns {
    member: TableColumn
}

structure TableColumn {
    /// Text in the heading row.
    @required
    heading: Template

    /// Text in each row, which templates read the row's element as `item`.
    @required
    text: Template

    /// Where the text sits across the column.
    align: TextAlign = "start"

    /// Lines a cell shows before its text is cut with an ellipsis.
    clamp: Lines = 1
}

enum TableSize {
    XSMALL = "xsmall"
    SMALL = "small"
    BASE = "base"
    LARGE = "large"
    XLARGE = "xlarge"
}

enum TextAlign {
    START = "start"
    CENTER = "center"
    END = "end"
}

/// A track that fills as far as `value` is of `max`, under a line with `label`
/// at its start and `figure` at its end. It fills what its parent fixes; where
/// its parent sizes it to content, it is 320 pixels wide.
structure ProgressBar {
    /// How far, as a number.
    @required
    value: Template

    /// The value of a full track, as a number.
    max: Template = "100"

    label: Template

    /// Text at the end, such as the value with its unit.
    figure: Template

    size: ProgressSize = "base"

    /// How dark the fill is: 1, 2 or 3.
    emphasis: Emphasis = 1

    grow: Boolean = false
}

/// `count` dots, the first `value` of them filled.
structure ProgressDots {
    /// How many dots, as a number from 1 to 64.
    @required
    count: Template

    /// How many are filled, as a number.
    @required
    value: Template

    /// Marks the dot after the filled ones as the step under way.
    current: Boolean = false

    size: ProgressSize = "base"
}

enum ProgressSize {
    XSMALL = "xsmall"
    SMALL = "small"
    BASE = "base"
    LARGE = "large"
}

/// Markdown in TRMNL's rich text: paragraphs apart by a blank line, `**strong**`,
/// `*emphasis*`, and lists. Headings are strong paragraphs; links, code and
/// images show their text. It is at most 640 pixels wide on an 800-pixel screen,
/// as TRMNL's rich text is.
structure RichText {
    @required
    text: Template

    size: RichTextSize = "base"

    /// Where the text sits, and how its lines align.
    align: TextAlign = "start"

    /// Space between paragraphs.
    gap: Gap = "medium"
}

enum RichTextSize {
    SMALL = "small"
    BASE = "base"
    LARGE = "large"
    XLARGE = "xlarge"
    XXLARGE = "xxlarge"
    XXXLARGE = "xxxlarge"
}

@length(min: 1, max: 4)
list ChartSeries {
    member: Series
}

structure Series {
    /// The values, as a list of numbers.
    @required
    values: Template

    name: Template
}

structure Each {
    /// The list, as a template.
    @required
    items: Template

    /// The content drawn for each element, which templates read as `item`.
    @required
    content: Component

    direction: Direction = "column"

    gap: Gap = "medium"

    /// Elements drawn at most; absent draws at most 100.
    limit: EachLimit

    /// Shown after the last element drawn when some are left out; reads `hidden`,
    /// the number left out.
    more: Template = "and {{ hidden }} more"

    /// Takes a share of the space left along its parent, as a stack does, so that
    /// elements that grow can fill it.
    grow: Boolean = false
}

@range(min: 1, max: 100)
integer EachLimit

/// `then` where `when` is true, `else` (or nothing) where it is not, so a
/// template can choose a style, such as inverting the cell of today.
structure Choose {
    /// Tested as Jinja's `if` tests a value: false, nothing, zero and empty text
    /// or lists are not true. Write the test as one `{{ expression }}`, which keeps
    /// its value; text around it makes the template text, which is true unless
    /// empty.
    @required
    when: Template

    @required
    then: Component

    /// Absent draws nothing in its place.
    else: Component
}

enum Direction {
    COLUMN = "column"
    ROW = "row"
}

enum Gap {
    NONE = "none"
    XSMALL = "xsmall"
    SMALL = "small"
    MEDIUM = "medium"
    LARGE = "large"
    XLARGE = "xlarge"
    XXLARGE = "xxlarge"
}

enum Align {
    START = "start"
    CENTER = "center"
    END = "end"
    STRETCH = "stretch"
}

enum Justify {
    START = "start"
    CENTER = "center"
    END = "end"
    BETWEEN = "between"
}

enum TextSize {
    SMALL = "small"
    BASE = "base"
    LARGE = "large"
    XLARGE = "xlarge"
    XXLARGE = "xxlarge"
}

enum LabelStyle {
    PLAIN = "plain"
    OUTLINE = "outline"
    UNDERLINE = "underline"
    GRAY = "gray"
    FILLED = "filled"
}

enum ValueSize {
    XXSMALL = "xxsmall"
    XSMALL = "xsmall"
    SMALL = "small"
    BASE = "base"
    LARGE = "large"
    XLARGE = "xlarge"
    XXLARGE = "xxlarge"
    XXXLARGE = "xxxlarge"
    MEGA = "mega"
    GIGA = "giga"
    TERA = "tera"
    PETA = "peta"
}

enum ImageFit {
    CONTAIN = "contain"
    COVER = "cover"
}

structure Screen {
    @required
    id: Id

    @required
    name: ScreenName

    @required
    document: ScreenDocument

    @required
    updatedAt: Instant
}

structure ScreenSummary {
    @required
    id: Id

    @required
    name: ScreenName

    @required
    updatedAt: Instant
}

list ScreenSummaries {
    member: ScreenSummary
}

/// The account's screens, by name, without their documents.
@readonly
@http(method: "GET", uri: "/manage/screens")
@optionalAuth
@tags(["mcp"])
operation ListScreens {
    output := {
        @required
        screens: ScreenSummaries
    }
}

/// Saves a new screen. A document that cannot be drawn is still saved, so draw it
/// with `PreviewScreen` first: a device whose screen fails stays on its last
/// image.
@http(method: "POST", uri: "/manage/screens", code: 201)
@optionalAuth
@tags(["mcp"])
operation CreateScreen {
    input := {
        @required
        name: ScreenName

        @required
        document: ScreenDocument
    }

    output := {
        @required
        screen: Screen
    }
}

/// The screen with its document.
@readonly
@http(method: "GET", uri: "/manage/screens/{screenId}")
@optionalAuth
@tags(["mcp"])
operation GetScreen {
    input := {
        @required
        @httpLabel
        screenId: Id
    }

    output := {
        @required
        screen: Screen
    }

    errors: [
        NotFound
    ]
}

/// Replaces the screen's name and document. Every device that shows it shows the
/// new document from its next check-in.
@idempotent
@http(method: "PUT", uri: "/manage/screens/{screenId}")
@optionalAuth
@tags(["mcp"])
operation UpdateScreen {
    input := {
        @required
        @httpLabel
        screenId: Id

        @required
        name: ScreenName

        @required
        document: ScreenDocument
    }

    output := {
        @required
        screen: Screen
    }

    errors: [
        NotFound
    ]
}

/// Deletes the screen and takes it out of every playlist; a device that slept on
/// it shows the server's own sleep screen instead, and one it was pinned on goes
/// back to its schedule.
@idempotent
@http(method: "DELETE", uri: "/manage/screens/{screenId}", code: 204)
@optionalAuth
@tags(["mcp"])
operation DeleteScreen {
    input := {
        @required
        @httpLabel
        screenId: Id
    }

    errors: [
        NotFound
    ]
}

/// Draws a screen document as a device with `display` would show it, with what
/// each data source answered.
@http(method: "POST", uri: "/manage/preview")
@optionalAuth
@tags(["mcp"])
operation PreviewScreen {
    input := {
        @required
        document: ScreenDocument

        @required
        display: Display

        /// The device whose name and size templates read; absent reads the
        /// display model's size and the name "Preview".
        deviceId: Id
    }

    output := {
        /// A PNG at the display's size; absent when the screen could not be drawn.
        image: Blob

        @required
        sources: SourceResults

        /// What kept the screen from being drawn as written.
        @required
        problems: Problems
    }

    errors: [
        NotFound
    ]
}

map SourceResults {
    key: Identifier
    value: SourceResult
}

union SourceResult {
    /// The JSON the source answered.
    data: Document

    /// Why the source gave no data.
    error: String
}

list Problems {
    member: Problem
}

structure Problem {
    @required
    message: String

    /// Where in the document, as a JSON Pointer such as `/content/column/children/0`.
    path: String
}
