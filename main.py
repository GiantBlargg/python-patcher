#!/usr/bin/python
from __future__ import print_function, unicode_literals, with_statement

import datetime
import os
import pprint
import socket
import sys

import common
import gameScanner
import httpGUI
import logger

try: input = raw_input
except NameError: pass

pp = pprint.PrettyPrinter(indent=4)

try:
	from urllib.request import urlopen, Request
	from urllib.error import HTTPError
except ImportError:
	from urllib2 import urlopen, Request, HTTPError

# If you double-click on the file in Finder on macOS, it will not open with a path that is near the .py file
# Since we want to properly find things like `./aria2c`, we should move to that path first.
dirname = os.path.dirname(sys.argv[0])
if dirname.strip():
	os.chdir(dirname)

if __name__ == "__main__":
	# Enable developer mode if we detect the program is run from the git repository
	# Comment out this line to simulate a 'normal' installation - files will be fetched from the web.
	if os.path.exists("installData.json"):
		common.Globals.DEVELOPER_MODE = True

	# redirect stdout to both a file and console
	# TODO: on MAC using a .app file, not sure if this logfile will be writeable
	#      could do a try-catch, and then only begin logging once the game path has been set?
	sys.stdout = logger.Logger(common.Globals.LOG_FILE_PATH)
	logger.setGlobalLogger(sys.stdout)
	sys.stderr = logger.StdErrRedirector(sys.stdout)

	print("\n\n------------------ Install Started On {} ------------------ ".format(datetime.datetime.now()))
	common.Globals.getBuildInfo()
	print("Installer Build Information:")
	print(common.Globals.BUILD_INFO)
	print("Installer is being run from: [{}]".format(os.getcwd()))

	if common.Globals.IS_WINDOWS and len(os.getcwd()) > 100:
		print("\n\n---------------------------------------------------------------------------------")
		print("ERROR: The path you are running the installer from is too long!")
		print("It is currently {} characters long, but must be less than 100 characters".format(len(os.getcwd())))
		print("Please move the installer to a shorter path. The current path is:")
		print(os.getcwd())
		common.exitWithError()

	# On Windows, check for non-ascii characters in hostname, which prevent the server starting up
	if common.Globals.IS_WINDOWS and not all(ord(c) < 128 for c in socket.gethostname()):
		print("-------------------------------------------------------------")
		print("ERROR: It looks like your hostname [{}] contains non-ASCII characters. This may prevent the installer from starting up.".format(socket.gethostname()))
		print("Please change your hostname to only contain ASCII characters, then restart the installer.")
		print("You can press ENTER to try to run the installer despite this problem.")
		print("-------------------------------------------------------------")
		input()

	def check07thModServerConnection():
		"""
		Makes sure that we can connect to the 07th-mod server
		(Patches will fail to download if we can't)
		"""
		try:
			testFile = urlopen(Request("http://07th-mod.com/", headers={"User-Agent": ""}))
			testFile.close()
		except HTTPError as error:
			print(error)
			print("Couldn't reach 07th Mod Server.  The installer will not be able to download patch files.")
			print("Note that we have blocked Japan from downloading (VPNs are compatible with this installer, however)")
			common.exitWithError()


	check07thModServerConnection()

	common.Globals.scanForExecutables()

	# Scan for moddable games on the user's computer before starting installation
	if common.Globals.DEVELOPER_MODE and os.path.exists("installData.json"):
		# Use local `installData.json` if it's there (if cloned from github)
		modList = common.getModList("installData.json")
	else:
		modList = common.getModList("https://raw.githubusercontent.com/07th-mod/python-patcher/master/installData.json")

	subModconfigList = []
	for mod in modList:
		for submod in mod['submods']:
			conf = gameScanner.SubModConfig(mod, submod)
			print(conf)
			subModconfigList.append(conf)

	installerGUI = httpGUI.InstallerGUI(subModconfigList)
	installerGUI.server_test()

	exit()
