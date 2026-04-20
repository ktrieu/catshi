#! /bin/bash

printf -v backup_date '%(%Y-%m-%d)T' -1

sqlite3 catshi.sqlite ".backup 'backups/$backup_date.sqlite'"



