/** Documents a new screen can start from: each shows one way of working, and each
 * draws as it stands. */
export interface Starter {
  id: string;
  name: string;
  summary: string;
  yaml: string;
}

export const STARTERS: Starter[] = [
  {
    id: "clock",
    name: "Date and time",
    summary: "No data source: the time of the drawing, formatted.",
    yaml: `titleBar:
  title: Today
  instance: "{{ device.name }}"
content:
  column:
    align: center
    justify: center
    children:
      - value:
          text: "{{ now() | timeformat(format='%H:%M') }}"
          size: giga
          tabular: true
      - title:
          text: "{{ now() | dateformat(format='%A, %-d %B') }}"
          size: xlarge
`,
  },
  {
    id: "weather",
    name: "Weather",
    summary: "A public JSON API, Open-Meteo, with values and a chart. Change the coordinates to yours.",
    yaml: `sources:
  weather:
    url: "https://api.open-meteo.com/v1/forecast?latitude=52.52&longitude=13.41&current=temperature_2m,relative_humidity_2m,wind_speed_10m&hourly=temperature_2m&forecast_hours=24&timezone=auto"
titleBar:
  title: Weather
  instance: Berlin
content:
  column:
    children:
      - grid:
          columns: 3
          cells:
            - content:
                item:
                  children:
                    - value:
                        text: "{{ weather.current.temperature_2m | round(1) }}°"
                        size: xlarge
                        fit: true
                    - label:
                        text: Temperature
            - content:
                item:
                  emphasis: 2
                  children:
                    - value:
                        text: "{{ weather.current.relative_humidity_2m }}%"
                        size: xlarge
                        fit: true
                    - label:
                        text: Humidity
            - content:
                item:
                  emphasis: 3
                  children:
                    - value:
                        text: "{{ weather.current.wind_speed_10m | round | int }} km/h"
                        size: xlarge
                        fit: true
                    - label:
                        text: Wind
      - divider: {}
      - label:
          text: "Next 24 hours: {{ weather.hourly.temperature_2m | min | round | int }} to {{ weather.hourly.temperature_2m | max | round | int }}°"
          size: small
          style: gray
      - lineChart:
          series:
            - values: "{{ weather.hourly.temperature_2m }}"
`,
  },
  {
    id: "home-assistant",
    name: "Home Assistant entity",
    summary:
      "A sensor read through Home Assistant's REST API, with a long-lived token the server holds as the secret HA_TOKEN. Change the address and entity to yours.",
    yaml: `sources:
  sensor:
    url: "http://homeassistant.local:8123/api/states/sensor.outdoor_temperature"
    headers:
      Authorization: "Bearer {{ secrets.HA_TOKEN }}"
titleBar:
  title: "{{ sensor.attributes.friendly_name }}"
content:
  column:
    align: center
    justify: center
    children:
      - value:
          text: "{{ sensor.state }} {{ sensor.attributes.unit_of_measurement }}"
          size: mega
          fit: true
      - label:
          text: "Updated {{ sensor.last_updated | datetimeformat(format='%H:%M') }}"
          style: gray
`,
  },
  {
    id: "list",
    name: "A list",
    summary: "Items from a JSON list, each in a row, with a count of those that do not fit.",
    yaml: `sources:
  posts:
    url: "https://jsonplaceholder.typicode.com/posts"
titleBar:
  title: Latest posts
content:
  each:
    items: "{{ posts }}"
    limit: 5
    gap: small
    content:
      item:
        index: "{{ item.id }}"
        children:
          - title:
              text: "{{ item.title | capitalize }}"
              clamp: 1
          - description:
              text: "{{ item.body }}"
              clamp: 1
`,
  },
  {
    id: "blank",
    name: "Blank",
    summary: "One title. Type to see what each component takes.",
    yaml: `content:
  title:
    text: Hello
    size: xlarge
`,
  },
];
