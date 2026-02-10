@echo off
REM InvestIQ Dash Frontend Startup Script for Windows

echo 🚀 InvestIQ Dashboard Startup
echo ==============================

REM Check if Python is installed
python --version >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ Python is not installed. Please install Python 3.8 or higher.
    pause
    exit /b 1
)

echo ✅ Python found

REM Check if virtual environment exists
if not exist "venv\" (
    echo 📦 Creating virtual environment...
    python -m venv venv
)

REM Activate virtual environment
echo 🔧 Activating virtual environment...
call venv\Scripts\activate.bat

REM Install/update requirements
echo 📥 Installing dependencies...
pip install -q --upgrade pip
pip install -q -r requirements.txt

REM Check if API server is running
echo 🔍 Checking API server...
curl -s http://localhost:3000/health >nul 2>&1
if %errorlevel% neq 0 (
    echo ⚠️  WARNING: API server not detected at http://localhost:3000
    echo    Please start the API server first:
    echo    cd .. ^&^& cargo run --release --bin api-server
    echo.
    set /p continue="Continue anyway? (y/n): "
    if /i not "%continue%"=="y" exit /b 1
)

REM Start the dashboard
echo.
echo 🎨 Starting InvestIQ Dashboard...
echo 📊 Dashboard will be available at: http://localhost:8050
echo ⏹️  Press Ctrl+C to stop
echo.

python app.py
pause
