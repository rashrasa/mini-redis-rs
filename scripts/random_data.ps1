
filter Text {
    $r = Get-Random -Maximum 1000
    "{`"Insert`": [test_$_, $r]}"
}

filter Join {
    $ofs = "-"
    -join $_
}



0..25 | Text | Join | Out-String | ncat 127.0.0.1 3000