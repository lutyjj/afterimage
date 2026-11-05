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
/// `secrets`.
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

    /// Its content once for each element of a list.
    each: Each
}

@length(min: 1, max: 32)
list Components {
    member: Component
}

/// Children size to their content; those with `grow` share the space left along
/// the stack's direction.
structure Stack {
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

structure Item {
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

structure Image {
    /// An `http`, `https` or `data` URL of a PNG, JPEG, GIF, WebP or SVG image.
    @required
    url: Template

    fit: ImageFit = "contain"

    grow: Boolean = true
}

structure Divider {}

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

    /// Elements drawn at most.
    limit: EachLimit

    /// Shown after the last element drawn when some are left out; reads `hidden`,
    /// the number left out.
    more: Template = "and {{ hidden }} more"
}

@range(min: 1, max: 100)
integer EachLimit

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

@readonly
@http(method: "GET", uri: "/manage/screens")
operation ListScreens {
    output := {
        @required
        screens: ScreenSummaries
    }
}

@http(method: "POST", uri: "/manage/screens", code: 201)
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

@readonly
@http(method: "GET", uri: "/manage/screens/{screenId}")
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

@idempotent
@http(method: "PUT", uri: "/manage/screens/{screenId}")
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

/// Deletes the screen and takes it out of every playlist.
@idempotent
@http(method: "DELETE", uri: "/manage/screens/{screenId}", code: 204)
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


/// The names of the secrets the deployment provides, which data source requests read
/// as `secrets.NAME`. The values never appear in an answer, but every account's
/// requests may send them anywhere: a deployment gives its secrets to every account
/// it lets sign in.
@readonly
@http(method: "GET", uri: "/manage/secrets")
operation ListSecretNames {
    output := {
        @required
        names: Names
    }
}
