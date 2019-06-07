# binrep


_Trusted binary repository_

The aim is to create a trusted repository of versioned binary artifacts accessible via file or network protocols (eg: HTTP). Binaries are signed by the uploader. Versions of binaries are tracked. 


## Version of binaries

Version needs to follow the regex: ```[0-9]+(\.[0-9]+){1,2}(-[0-9A-Za-z_-]+)*```

Example of valid versions: ```1```,  ```1.0```, ```1.2.3```,  ```1.6-12-fixed```...

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
- concatenate the name, checksum, and checksum_method of each file, in the order they appear in the files field,
- convert the string to UTF-8 bytes
- sign the UTF-8 bytes with the private key and the signature_method
- output the result to base64.

### Optional .json metadata files

For maximum interoperability sane metadata files should be mirrored with json files following the same structure

### Available hashing algorithm

`SHA256`

### Available signature method

`HMAC_SHA256` publisher & repository readers can agree on what key to use by using the key_id field.


