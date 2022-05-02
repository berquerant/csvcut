# csvcut

```
❯ csvcut --help
csvcut
Cut out selected portions of each line of csv from stdin

USAGE:
    csvcut [OPTIONS] --target <TARGET>

OPTIONS:
    -d, --delimiter <DELIMITER>
            Use DELIMITER as the field delimiter character instead of the ','

            [default: ,]

    -f, --target <TARGET>
            Selected portions.

            Single:
            ```
            ❯ (echo 'a,b,c';echo '2,3,4';echo '11,12,13') | csvcut -f 1
            a
            2
            11
            ```
            Left limit:
            ```
            ❯ (echo 'a,b,c';echo '2,3,4';echo '11,12,13') | csvcut -f 2-
            b,c
            3,4
            12,13
            ```
            Right limit:
            ```
            ❯ (echo 'a,b,c';echo '2,3,4';echo '11,12,13') | csvcut -f -2
            a,b
            2,3
            11,12
            ```
            Interval:
            ```
            ❯ (echo 'a,b,c,d';echo '1,2,3,4';echo '11,12,13,14') | csvcut -f 2-3
            b,c
            2,3
            12,13
            ```
            Single + Right:
            ```
            ❯ (echo 'a,b,c,d';echo '1,2,3,4';echo '11,12,13,14') | csvcut -f 1,3-
            a,c,d
            1,3,4
            11,13,14
            ```
            Single + Right, ignore headers:
            ```
            ❯ (echo 'a,b,c,d';echo '1,2,3,4';echo '11,12,13,14') | csvcut -f 1,3- --header
            1,3,4
            11,13,14
            ```

    -h, --help
            Print help information

        --header
            Read or ignore headers.  See --json and --target

    -j, --json
            Print results as json.

            e.g.
            ```
            ❯ (echo 'a,b,c';echo '2,3,4';echo '11,12,13') | csvcut -f 2 --json
            ["b"]
            ["3"]
            ["12"]
            ❯ (echo 'a,b,c';echo '2,3,4';echo '11,12,13') | csvcut -f 2 --json --header
            {"b":"3"}
            {"b":"12"}
            ❯ (echo '"a,b","c","d,e,f"';echo '"1","2,3","4"';echo '"11","12","13,14,15"') | csvcut
            -f 1,3 --json
            ["a,b","d,e,f"]
            ["1","4"]
            ["11","13,14,15"]
            ```
```
