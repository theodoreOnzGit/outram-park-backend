# chem-eng-real-time-process-control-simulator
A real-time process control simulator library for chemical engineering 
and other engineering.

## to run 

```bash
cargo run 
```
To watch:
```bash
cargo watch -x run --ignore "*.csv"
```

## Documentation

TBD. Theory of basic items is in my PhD thesis (to be published later).

Citations appreciated.

## Licenses 

This crate is released under the Apache 2.0 License. 

Copyright [2023] [Theodore Kay Chen Ong, Professor Per F. Peterson,
University of California, Berkeley
Thermal Hydraulics Lab, Repository Contributors and 
Singapore Nuclear Research and Safety Initiative (SNRSI)]

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.

I use crates such as approx, csv, thiserror and uom. The licenses 
are located in the licenses_of_dependencies folder.


## References 

I used some of these references to develop this library:

Seborg, Dale E., Thomas F. Edgar, Duncan A. Mellichamp, and 
Francis J. Doyle III. Process dynamics and control. John Wiley & Sons, 2016.

Green, D. W., & Perry, R. H. (2008). Perry's chemical engineers' 
handbook. McGraw-Hill Education.
