from __future__ import unicode_literals

import glob
import os
import re
import shutil
import subprocess
import sys
import datetime
import platform

print("--- Running 07th-Mod Installer Build using Python {} ---".format(sys.version))

BUILD_LINUX_MAC = True
# If user specified which platform to build for, use that platform. Otherwise, attempt to detect platform automatically.
if len(sys.argv) == 2:
	if "win" in sys.argv[1].lower():
		BUILD_LINUX_MAC = False
else:
	BUILD_LINUX_MAC = not (platform.system() == "Windows")

print(f"Building Linux Mac: {BUILD_LINUX_MAC}")

IS_WINDOWS = sys.platform == "win32"

def call(args, **kwargs):
	print("running: {}".format(args))
	retcode = subprocess.call(args, shell=IS_WINDOWS, **kwargs) # use shell on windows
	if retcode != 0:
		exit(retcode)


def try_remove_tree(path):
	try:
		if os.path.isdir(path):
			shutil.rmtree(path)
		else:
			os.remove(path)
	except FileNotFoundError:
		pass


def zip(input_path, output_filename):
	try_remove_tree(output_filename)
	call(["7z", "a", output_filename, input_path])


def tar_gz(input_path, output_filename: str):
	try_remove_tree(output_filename)
	tempFileName = re.sub("\.gz", "", output_filename, re.IGNORECASE)
	call(["7z", "a", tempFileName, input_path])
	call(["7z", "a", output_filename, tempFileName])
	os.remove(tempFileName)

print("\nTravis python build script started\n")

# first, copy the files we want into a staging folder
staging_folder = 'travis_installer_staging'
output_folder = 'travis_installer_output'
bootstrap_copy_folder = 'travis_installer_bootstrap_copy'

# No wildcards allowed in these paths to be ignored
ignore_paths = [
	staging_folder,
	output_folder,
	bootstrap_copy_folder,
	'JSONValidator',
	'installData.json',
	'versionData.json',
	'cachedDownloadSizes.json',
	'httpGUI/node_modules',
	'bootstrap',
	'.git',
	'.idea',
	'.gitignore',
	'.travis.yml',
	'__pycache__',
	'news',
	'install_loader',
	'installerTests',
	'Onikakushi Ch.1 Downloads',
	'Watanagashi Ch.2 Downloads',
	'Tatarigoroshi Ch.3 Downloads',
	'Himatsubushi Ch.4 Downloads',
	'Meakashi Ch.5 Downloads',
	'Tsumihoroboshi Ch.6 Downloads',
	'Minagoroshi Ch.7 Downloads',
	'Console Arcs Downloads',
	'Umineko Question (Ch. 1-4) Downloads',
	'Umineko Answer (Ch. 5-8) Downloads',
	'Umineko Tsubasa Downloads',
	'Umineko Hane Downloads',
	'INSTALLER_LOGS',
]
ignore_paths_realpaths = set([os.path.realpath(x) for x in ignore_paths])

def ignore_filter(folderPath, folderContents):
	ignored_children = []

	for child in folderContents:
		fullPath = os.path.join(folderPath, child)
		if os.path.realpath(fullPath) in ignore_paths_realpaths:
			ignored_children.append(child)

	# ignoredChildrenString = f'Ignoring: {ignored_children}' if ignored_children else ''
	print(f'\nCopying Folder: [{folderPath}]')
	for child in ignored_children:
		print(f' - Ignored [{child}]')

	return ignored_children #ignore_patterns_func(folderPath, folderContents)

try_remove_tree(bootstrap_copy_folder)
try_remove_tree(output_folder)
try_remove_tree(staging_folder)

# Fix for Python 3.8 - For copytree to ignore folders correctly, they must exist before the copy process begins
# Make sure the output folder exists
os.makedirs(output_folder, exist_ok=True)
os.makedirs(staging_folder, exist_ok=True)
os.makedirs(bootstrap_copy_folder, exist_ok=True)

# copy bootstrap folder to a temp folder
shutil.copytree('bootstrap', bootstrap_copy_folder, dirs_exist_ok=True)

# copy all files in the root github directory, except those in ignore_patterns
shutil.copytree('.', staging_folder, ignore=ignore_filter, dirs_exist_ok=True)

# Save the build information in the staging folder. Will later be read by installer.
with open(os.path.join(staging_folder, 'build_info.txt'), 'w', encoding='utf-8') as build_info_file:
	build_info_file.write(f'Build Date: {datetime.datetime.now()}\n')
	build_info_file.write(f'Git Tag (Version): {os.environ.get("TRAVIS_TAG")}\n')

# now, copy the staged files into each os's bootstrap folder's install_data directory
for osBootStrapPath in glob.glob(f'{bootstrap_copy_folder}/*/'):
	print("processing", osBootStrapPath)
	# osBootStrapPath = os.path.join(bootStrapRoot, osFolderName)
	osInstallData = os.path.join(osBootStrapPath, 'install_data')
	if IS_WINDOWS:
		call(['xcopy', '/E', '/I', '/Y', staging_folder, osInstallData])
	else:
		call(['cp', '-r', staging_folder + '/.', osInstallData])

# FOR WINDOWS ONLY: Extract the python archive, then delete the archive
call(["7z", "x", f'./{bootstrap_copy_folder}/higu_win_installer_32/install_data/python_archive.7z', f'-o./{bootstrap_copy_folder}/higu_win_installer_32/install_data/'])
try_remove_tree(f'./{bootstrap_copy_folder}/higu_win_installer_32/install_data/python_archive.7z')

# RELATIVE PATHS MUST CONTAIN ./
if BUILD_LINUX_MAC:
	os.rename(f'./{bootstrap_copy_folder}/higu_linux64_installer/', f'./{bootstrap_copy_folder}/07th-Mod_Installer_Linux64/')
	tar_gz(f'./{bootstrap_copy_folder}/07th-Mod_Installer_Linux64/', os.path.join(output_folder, '07th-Mod.Installer.linux.tar.gz'))
# zip(f'./{bootstrap_copy_folder}/higu_win_installer/', os.path.join(output_folder, '07th-Mod.Installer.win64.zip'))
# zip(f'./{bootstrap_copy_folder}/higu_win_installer_32/', os.path.join(output_folder, '07th-Mod.Installer.win.zip'))

if not BUILD_LINUX_MAC:
	# Create an archive of the contents install_data folder (no subfolder)
	loader_src_folder = 'install_loader/src'
	tar_path = os.path.join(loader_src_folder, 'install_data.tar')
	xz_path = tar_path + '.xz'
	try_remove_tree(tar_path)
	try_remove_tree(xz_path)
	call(['7z', 'a', '-aoa', tar_path, f'./{bootstrap_copy_folder}/higu_win_installer_32/install_data/*'])
	call(['7z', 'a', '-aoa', xz_path, tar_path])

	# Compile the rust loader
	# If not using a manifest file, DO NOT put the words "install", "patch", "update", etc. in the filename,
	# or else windows will force running the .exe as administrator
	# https://stackoverflow.com/questions/31140051/windows-force-uac-elevation-for-files-if-their-names-contain-update
	# If using msvc linker, embed a manifest/change msvc linker options, as per
	# https://www.reddit.com/r/rust/comments/8tooi0/hey_rustaceans_got_an_easy_question_ask_here/e1lk7tw?utm_source=share&utm_medium=web2x
	loader_exe_name = '07th-Mod.Installer.Windows.exe'
	call(['cargo', 'rustc', '--release', '--', '-C', 'link-arg=/MANIFEST:embed'], cwd=loader_src_folder)

	# Copy the exe to the final output folder
	shutil.copy('install_loader/target/release/install_loader.exe', os.path.join(output_folder, loader_exe_name))

# NOTE: mac zip doesn't need subdir - use '/*' to achieve this
if BUILD_LINUX_MAC:
	os.rename(f'./{bootstrap_copy_folder}/higu_mac_installer/', f'./{bootstrap_copy_folder}/07th-Mod_Installer_Mac/')
	zip(f'./{bootstrap_copy_folder}/07th-Mod_Installer_Mac/*', os.path.join(output_folder, '07th-Mod.Installer.mac.zip'))

try_remove_tree(staging_folder)
try_remove_tree(bootstrap_copy_folder)
