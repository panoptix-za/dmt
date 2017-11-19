## Intro

`dmt: DevOps Multi-Tool`

`dmt` is a  work in progress, at it's heart it is a cli based template rendering system, what isn't well known is how incredibly powerful such a system can be when put to creative uses (more on this in the future).


See our rudimentary docs [Docs](docs/contexts.md) 


## To Do

- Sharpen up documentation

    - add lots of example projects

- Add features
    
    - advanced template manipulation
        
        - whitespace, headers, footers, merge multiple files
        
        - render and execute shell scripts
    
    - data sources
        
        - prompts
        - vault/secrets
        - toml
        - api
        - system information (ip, hostname, etc)
        
    - watch mode 
    
        - run jobs (`.dmt.job`)
        
    - logging
    
    - context sharing
    
        - load/save context in consul, etcd
        
        - dmt to dmt through gossip
        
    - embedded admin interface
        

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

### Code of Conduct

Contribution to the `dmt` crate is organized under the terms of the Contributor Covenant, the maintainer of `dmt`, @stephanbuys, promises to intervene to uphold that code of conduct.
