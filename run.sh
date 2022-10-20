#!/bin/bash -eux

curl --parallel --trace dump --http2-prior-knowledge http://localhost:6400/ \
	-: http://localhost:6400/ \
	-: http://localhost:6400/
