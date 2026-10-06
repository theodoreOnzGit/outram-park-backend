# synthetic2020example

```toml
[kovan]
id = "synthetic2020example"
kind = "paper"
created = "2026-08-31T15:04:32+08:00"
modified = "2026-08-31T15:04:32+08:00"

[classification]
topics = ["htgrs/materials"]
```

```latex
@article{synthetic2020example,
  author  = {Example, Ada and Placeholder, Ben},
  title   = {A Synthetic Paper Used Only as a Test Fixture},
  journal = {Journal of Fixtures},
  year    = {2020}
}
```

## Summary

A synthetic paper header. Nothing here is from a real note.

# Graphite temperature assumption

```toml
connections = ["a1b2c3d4e5f6"]

[kovan]
id = "graphite-temperature-assumption"
kind = "annotation"
created = "2026-08-31T15:04:32+08:00"
modified = "2026-08-31T15:04:32+08:00"

[source]
page = 87
region = [
    0.214,
    0.341,
    0.721,
    0.508,
]

[classification]
topics = ["htgrs/materials"]
```

Graphite temperature here appears to represent nominal operating conditions.

## A sub-heading inside the annotation

More prose; `##` never starts a new artifact.

# Fig. 3 specific heat

```toml
[kovan]
id = "fig-3-specific-heat"
kind = "digitised_graph"
created = "2026-09-24T10:00:00+08:00"
modified = "2026-09-24T10:00:00+08:00"

[source]
page = 12

[extraction]
method = "manual_digitisation"
figure = "Fig. 3"
x_label = "T (K)"
y_label = "cp (J/kg/K)"
review = "UNREVIEWED — points not yet human-verified"
```

### start of data series

### Series: sample A

```csv
T (K),cp (J/kg/K)
300,710
600,1180
```

### Series: sample B

```csv
T (K),cp (J/kg/K)
300,700
600,1160
```

### end of series

# Pages 42 to 48 matter

```toml
[kovan]
id = "pages-42-48"
kind = "source_reference"
created = "2026-09-01T09:00:00+08:00"
modified = "2026-09-01T09:00:00+08:00"
reviewed = "2026-09-02T09:00:00+08:00"

[source]
pages = [
    42,
    48,
]
```
