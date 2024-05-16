## 0.15.5

- upgrade dependencies (shellexpand & transitive deps)

## 0.15.3 / 0.15.4

- upgrade all dependencies
- musl build

## 0.15.2

- add read timeout whe pulling files from S3

## 0.15.1

- update deps (indicatif, tokio, rusoto*)

## 0.15.0

- update deps (semver, tokio, rusoto)
- BREAKING: semver crate updated its policy about prereleases
- temporary directories used to pull files are now created in the destination directory
  to workaround what seems to be a Rust bug on libc error reporting on cross links between
  filesystem

## 0.14.1 (0.14.0 yanked)

- going full async
- dramatic network performance improvment (100x faster transfers)

## 0.13.0

- upgrade to tokio 1.2 ecosystem

## 0.12.2

- Add context to binrep-batch error when reading batch configuration files
