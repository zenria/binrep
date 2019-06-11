# binrep

## Summary

_a repository manager for versioned binary artifacts_

Binrep is a repository manager of versioned binary artifacts. It supports 
storing the repository on local or network filesystem as well as AWS S3 backend. 
Binrep can be used to safely distribute binaries produced by a CI/CD build system. 

It is typically used to distribute compiled binaries, fat jars, zip files regardless of target
system. It's purpose is not to replace traditional package repositories nor docker registry but to
offer an alternative when using those is not convenient.

## Status

Binrep is still a work in progress.  

## Example 

```bash
# push the binrep-bin artifact v1.0.0 to the repository containing the target/release/binrep binray file
# directories are flattened
binrep push binrep-bin 1.0.0 target/release/binrep

# pull the binrep-bin files in the ~/.bin directory
binrep pull binrep-bin 1.0.0 ~/.bin

# latest version has a special meaning: it pulls the latest version according to semver.
binrep pull binrep-bin latest ~/.bin

# version can also be a requirement: https://docs.rs/semver/0.9.0/semver/#requirements
binrep pull binrep-bin "^1.0" ~/.bin

# keep the binaries in sync with the requirement ; download only binaries if needed
# metadata are kept in the destination directoy in a file named ".binrep-bin.sane"
# this command is typically used for continuous delivery
binrep sync binrep-bin latest ~/.bin
# will exec the given command if a new version has been successfully pulled
binrep sync haproxy-config latest /etc/haproxy --exec "sudo service haproxy reload"
```

## What is an artifact?

An artifact is a named versioned collection of binary files. 

Artifact names must only contain alphanumeric characters and `_-.`.  

Version needs to follow semver 2.0 https://semver.org/spec/v2.0.0.html format. 

Each artifact version can contains arbritraty number of files. 


## Metadata file format

Metadata file format follows the SANE file format: https://github.com/z0mbie42/sane

## Repository structure

```ROOT`` always represent the base folder/url of the repository.

Tha basic structure is: 
```yml
ROOT/:
  - actifacts.sane
  - artifact1/:
    - latest.sane
    - versions.sane
    - 1.0/:
      - artifact.sane
      - some_file1
      - some_file2
    - 1.1/:
    - 1.2.3/:
    ...
```

### artifacts.sane

List available artifacts by name:
```sane
artifacts=["artifact1", "artifact2"]
``` 
It should directly reflects the list of subdirectories inside the repository ```ROOT```. This files exists to be able to use protocols that does not supports subdirectories listing (eg: HTTP).

### latest.sane metatada

Contains the latest version:
```sane
latest_version="1.2.3"
```

### versions.sane

Contains the list of all available versions:
```sane
versions=["1.0","1.1","1.2.3"]
```
It should directly reflects the list of subdirectories inside the repository an artifact directory. This files exists to be able to use network protocols that does not supports subdirectories listing (eg: HTTP).

### artifact.sane metadata

Contains the list of binary files for the version with checksums and signatures.
```sane
version="1.2.3"
files=[
    {
        name="some_file1",
        checksum="abcabcabc1234513545",
        checksum_method="TBD",
    }
]
signature = {
    key_id="ABCDEF",
    signature="abcdefacbdef123456789123456789",
    signature_method="TBD"
}
```

Signature is generated as follow: 
- concatenate the name and checksum of each file, in the order they appear in the files field,
- convert the string to UTF-8 bytes
- sign the UTF-8 bytes with the private key and the signature_method
- output the result to base64.

### Optional .json metadata files

For maximum interoperability sane metadata files should be mirrored with json files following the same structure

### Available hashing algorithm

`SHA256`

### Available signature method

`HMAC_SHA256` publisher & repository readers can agree on what key to use by using the key_id field.


## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

