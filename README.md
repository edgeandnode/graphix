# Graph IXI

**Note: This software is under heavy development right now. Things can break at
any time.**

A system for cross-checking indexing and query results across [Graph
Nodes](https://github.com/graphprotocol/graph-node) and
[Indexers](https://github.com/graphprotocol/indexer).

_IXI_ stands for _indexer or integrity cross-checking_. It is designed to detect
inconsistencies in indexing results, with the help of so-called proofs of
indexing (POIs), and query results and allow anyone to view cross-checking
results both at a glance using high-level views as well as in-depth using
detailed reports that are intended to make debugging and dispute resolution
easy.

Graph IXI supports three modes:

1. **Testing:** Cross-checks specific environments, like local Graph Nodes or specific indexers.
2. **Network:** Cross-checks all indexers on The Graph Network.
3. **Indexer:** Cross-checks one's own indexer with indexers on The Graph Network.

## Build

Simply build and install all executables:

```sh
cargo install

cd frontend/
trunk build --release
cd dist/
# now copy the HTML and JS from this directory to a web server
```

## Usage

During development, run the following commands. They will automatically restart
all processes as you make changes to the code.

```sh
# Run the cross-checker
cargo watch -x 'run -p graph-ixi-cross-checker -- --config examples/testing.yml'

# Run the API server
cargo watch -x 'run -p graph-ixi-api-server -- --port 3030'

# Run the web frontend
cd frontend && trunk serve
```

In production, run these:

```sh
graph-ixi-cross-checker --config /path/to/your/config.yml

graph-ixi-api-server --port <port>

cd frontend/
trunk build --release
cd dist/
# now serve the HTML and JS from this directory somehow
```

## Implementation Status

- [ ] Mode configuration files
  - [x] Testing
  - [ ] Network
  - [ ] P2P
- [ ] Cross-checking
  - [ ] Indexing
    - [x] Monitor indexing statuses
    - [x] Monitor POIs for common blocks
    - [x] Write POIs to a POI database
    - [x] Cross-check POIs
    - [ ] Generate detailed report data
  - [ ] Querying
    - tbd
- [ ] Custom cross-checking (largely tbd)
  - [ ] Indexing performance (desirable)
- [ ] Frontend
  - [ ] Indexing
    - [ ] POI explorer
    - [ ] POI cross-checking overview(s)
    - [ ] Detailed report views
  - [ ] Querying
    - tbd

# Copyright

&copy; 2021 Edge & Node Ventures, Inc.

Graph IXI is dual-licensed under the MIT license and the Apache License, Version
2.0.

Unless required by applicable law or agreed to in writing, software distributed
under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
CONDITIONS OF ANY KIND, either expressed or implied. See the License for the
specific language governing permissions and limitations under the License.
