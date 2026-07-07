@echo off
cd /d C:\Users\shiva\workspace\leanstack
echo Starting onpush generate at %date% %time% > onpush_run.log
npx --yes onpush@latest generate --verbose --full --type product-overview --model claude-sonnet-4-5 >> onpush_run.log 2>&1
echo Finished at %date% %time% with exit code %ERRORLEVEL% >> onpush_run.log
