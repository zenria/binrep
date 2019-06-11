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
# metadata are kept in the destination directoy in a file named ".binrep-bin_sync.sane"
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

## Configuration

### Location of config file

Configuration can be provided with the `-c` or `--config` flag. If no configuration is provided, binrep will 
search in `~/.binrep/confif.sane` and `/etc/binrep/config.sane`.

### Configuration

Sample config file for pulling artifacts:
```sane
[backend]
type = "file"
root = "/mnt/test-repo"

# List available keys for HMAC SHA256 signature method
[hmac_keys]
"test-key" = "okIy37MEOC8yCkCEcMbyVCYEWNZT7IV5wr+qQxFlYR0="

```

For publishing artifact additional config is needed: 
```sane
[backend]
type = "file",
root = "./test-repo"

# List available keys for HMAC SHA256 signature method
[hmac_keys]
"test-key" = "okIy37MEOC8yCkCEcMbyVCYEWNZT7IV5wr+qQxFlYR0="

# Parameters used when publishing artifacts
[publish_parameters]
# Signature method when publishing
signature_method = "HMAC_SHA256",
# Checksum method when publishing
checksum_method = "SHA256",
# Reference to HMAC SHA256 key when publishing
hmac_signing_key = "test-key",

```
### Available hashing algorithm

`SHA256`, `SHA384`, `SHA512`

### Available signature method

`HMAC_SHA256`, `HMAC_SHA384`, `HMAC_SHA512` publisher & repository readers can agree on what key to use by using the key_id field.

### Shared HMAC-SHAxxx secret key

Artifacts are hashed and the associated metadata (filename+hash) is signed using some crypto signature algorithm. 
Currently, binrep only supports HMAC-SHAxxx signature, pull & push clients must share a secret key to verify metadata
integrity. The signing key has an id thus, multiple keys can be configured.

The key consists of 32/48/64 random bytes. It must be base64 encoded to be included in the binrep config files. 
It can be generated using the following command: 
````bash
# for HMAC-SHA256
openssl rand -base64 32
# for HMAC-SHA384
openssl rand -base64 48
# for HMAC-SHA512
openssl rand -base64 64
````

### AWS S3 configuration

Binrep uses the same credentials as aws cli commands. If nothing configured it will get the default credentials.

Backend sample section: 
```sane
[backend]
type = "s3"
bucket = "my-binrep-artifacts"
# region is mandatory as it determines the aws api gateway
region = "eu-west-3"
# optional profile name
profile = "gitlabci"    
```


 

## Internals

### Metadata file format

Metadata file format follows the SANE file format: https://github.com/z0mbie42/sane

### Repository structure

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

#### artifacts.sane

List available artifacts by name:
```sane
artifacts=["artifact1", "artifact2"]
``` 
It should directly reflects the list of subdirectories inside the repository ```ROOT```. This files exists to be able to use protocols that does not supports subdirectories listing (eg: HTTP).

#### versions.sane

Contains the list of all available versions:
```sane
versions=["1.0","1.1","1.2.3"]
```
It should directly reflects the list of subdirectories inside the repository an artifact directory. This files exists to be able to use network protocols that does not supports subdirectories listing (eg: HTTP).

#### artifact.sane metadata

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

