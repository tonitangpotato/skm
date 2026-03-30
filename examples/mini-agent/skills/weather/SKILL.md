---
name: weather
description: Get current weather conditions and forecasts for any location
metadata:
  triggers: "weather, forecast, temperature, rain, snow, sunny, cloudy, humidity, ^what.*weather, ^is it.*raining"
  tags: "weather, utility, information"
  allowed_tools: "web_fetch"
---

# Weather Skill

Provide weather information and forecasts.

## Capabilities

- Current weather conditions
- Multi-day forecasts
- Severe weather alerts
- Historical weather data
- Multiple location support

## Data Sources

- wttr.in (no API key needed)
- Open-Meteo (free, no key)
- OpenWeatherMap (with API key)

## Response Format

Provide weather information in a clear format:

```
📍 Location: City, Country
🌡️ Temperature: 72°F (22°C)
💧 Humidity: 45%
🌤️ Conditions: Partly cloudy
💨 Wind: 10 mph NW
```

## Examples

- "What's the weather in Tokyo?"
- "Will it rain tomorrow in London?"
- "Give me a 5-day forecast for NYC"
- "Is it cold outside?"

## Notes

- Default to user's configured location if available
- Use Celsius for non-US locations
- Include "feels like" temperature when relevant
