#!/usr/bin/env python
# coding=utf-8

import os
import copy
import sys
import math

def make_config():
	nid = int(sys.argv[2])
	path = os.path.join(sys.argv[1],"node" + str(nid))
	ip_list = (sys.argv[4]).split(',')
	duration = int(sys.argv[5])
	hz = int(sys.argv[6])
	port = ip_list[nid].split(':')[1]
	config_name = "config"
	size = int(sys.argv[3])
	dump_path = os.path.join(path, config_name)
	f = open(dump_path, "w")
	f.write("id_card = " + str(nid) + "\n")
	f.write("port = " + port + "\n")
	f.write("max_peer = " + str(size - 1) + "\n")
	f.write("duration = " + str(duration) + "\n")
	f.write("hz = " + str(hz) + "\n")
	f.write("epoch_len = " + str(10) + "\n")
	f.write("start_time = " + str(1) + "\n")
	secret_path = os.path.join(path, "miner_privkey")
	secret_key = open(secret_path, "r")
	key = secret_key.read()
	f.write("miner_private_key = \"" + key + "\"\n")
	secret_path = os.path.join(path, "signer_privkey")
	secret_key = open(secret_path, "r")
	key = secret_key.read()
	secret_key.close()
	f.write("signer_private_key = \"" + key + "\"\n")
	ids=range(size)
	ip_list = zip(ids, ip_list)
	del ip_list[nid]
	for (id, addr) in ip_list :
		addr_list = addr.split(':')
		f.write("[[peers]]" + "\n")
		f.write("id_card = " + str(id) + "\n")
		ip = addr_list[0]
		f.write("ip = \"" + ip + "\"\n")
		port = addr_list[1]
		f.write("port = " + port + "\n")
	
    #generate keypairs
	signer_auth_path = os.path.join(sys.argv[1], "signer_authorities")
	signer_auth = open(signer_auth_path, "r")
	miner_auth_path = os.path.join(sys.argv[1], "miner_authorities")
	miner_auth = open(miner_auth_path, "r")

	while True:
		signer_key = signer_auth.readline().strip('\n')
		miner_key = miner_auth.readline().strip('\n')
		if (not signer_key) or (not miner_key):
			break
		f.write("[[keygroups]]" + "\n")
		f.write("miner_public_key = \"" + miner_key + "\"\n")
		f.write("signer_public_key = \"" + signer_key + "\"\n")

	signer_auth.close()
	miner_auth.close()
	f.close()

make_config()
