#!/usr/bin/env python
# coding=utf-8

import os
import sys

def make_config():
    nid = int(sys.argv[2])
    path = os.path.join(sys.argv[1],"node" + str(nid))
    keypairs_path = os.path.join(sys.argv[1], "bls.keypairs")
    ntp_path = os.path.join(sys.argv[1], "ntp_servers")
    keypairs_f = open(keypairs_path, "r")
    keypairs = keypairs_f.readlines()
    ip_list = (sys.argv[4]).split(',')
    steps = int(sys.argv[5])
    nps = int(sys.argv[6])
    port = ip_list[nid].split(':')[1]
    config_name = "config"
    size = int(sys.argv[3])
    dump_path = os.path.join(path, config_name)
    f = open(dump_path, "w")
    f.write("id_card = " + str(nid) + "\n")
    f.write("port = " + port + "\n")
    f.write("max_peer = " + str(size - 1) + "\n")
    f.write("steps = " + str(steps) + "\n")
    f.write("nps = " + str(nps) + "\n")
    f.write("epoch_len = " + str(10) + "\n")
    f.write("start_time = " + str(1) + "\n")
    key = keypairs[nid * 3]
    f.write("miner_private_key = " + key)
    secret_path = os.path.join(path, "signer_privkey")
    secret_key = open(secret_path, "r")
    key = secret_key.read()
    secret_key.close()
    f.write("signer_private_key = \"" + key + "\"\n")
    ntp_servers_f = open(ntp_path, "r")
    ntp_servers = ntp_servers_f.read()
    ntp_servers_f.close()
    f.write("ntp_servers = " + ntp_servers + "\n")
    f.write("buffer_size = 5\n")
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

    i = 1;
    while True:
        signer_key = signer_auth.readline().strip('\n')
        proof_key = keypairs[i]
        proof_g = keypairs[i+1]
        if (not signer_key) or (not proof_key):
            break
        f.write("[[keygroups]]" + "\n")
        f.write("proof_public_key = " + proof_key)
        f.write("proof_public_g = " + proof_g)
        f.write("signer_public_key = \"" + signer_key + "\"\n")
        i += 3

    signer_auth.close()
    keypairs_f.close()
    f.close()

make_config()
